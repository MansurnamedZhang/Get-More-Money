use super::{AppState, common};
use crate::error::{AppError, AppResult};
use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{
    Json, Router,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::{Duration, SecondsFormat, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const SESSION_COOKIE: &str = "investment_session";
const SESSION_DAYS: i64 = 30;

#[derive(Debug, Serialize, sqlx::FromRow)]
struct UserView {
    id: String,
    username: String,
    display_name: String,
}

#[derive(Debug, Serialize)]
struct AuthStatus {
    setup_required: bool,
    authenticated: bool,
    user: Option<UserView>,
}

#[derive(Debug, Deserialize)]
struct SetupInput {
    username: String,
    display_name: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct LoginInput {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct AuthResponse {
    user: UserView,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/status", get(status))
        .route("/auth/setup", post(setup))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
}

pub async fn require_auth(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if !state.auth_required {
        return next.run(request).await;
    }
    match authenticated_user(&state, request.headers()).await {
        Ok(Some(_)) => next.run(request).await,
        Ok(None) => AppError::Unauthorized("请先登录".to_owned()).into_response(),
        Err(error) => error.into_response(),
    }
}

async fn status(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<AuthStatus>> {
    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await?;
    let user = authenticated_user(&state, &headers).await?;
    Ok(Json(AuthStatus {
        setup_required: user_count == 0,
        authenticated: user.is_some(),
        user,
    }))
}

async fn setup(
    State(state): State<AppState>,
    Json(input): Json<SetupInput>,
) -> AppResult<Response> {
    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await?;
    if user_count > 0 {
        return Err(AppError::Conflict("本地管理员已经创建".to_owned()));
    }
    let username = validate_username(&input.username)?;
    let display_name = common::required_text(&input.display_name, "显示名称", 80)?;
    validate_password(&input.password)?;
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(input.password.as_bytes(), &salt)
        .map_err(|_| AppError::Validation("密码处理失败".to_owned()))?
        .to_string();
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO users (id, username, display_name, password_hash) VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&username)
    .bind(&display_name)
    .bind(password_hash)
    .execute(&state.db)
    .await?;
    create_session_response(
        &state,
        UserView {
            id,
            username,
            display_name,
        },
        StatusCode::CREATED,
    )
    .await
}

async fn login(
    State(state): State<AppState>,
    Json(input): Json<LoginInput>,
) -> AppResult<Response> {
    let username = validate_username(&input.username)?;
    let row = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, username, display_name, password_hash FROM users WHERE username = ? COLLATE NOCASE",
    )
    .bind(username)
    .fetch_optional(&state.db)
    .await?;
    let Some((id, username, display_name, password_hash)) = row else {
        return Err(AppError::Unauthorized("用户名或密码错误".to_owned()));
    };
    let parsed = PasswordHash::new(&password_hash)
        .map_err(|_| AppError::Unauthorized("用户名或密码错误".to_owned()))?;
    if Argon2::default()
        .verify_password(input.password.as_bytes(), &parsed)
        .is_err()
    {
        return Err(AppError::Unauthorized("用户名或密码错误".to_owned()));
    }
    create_session_response(
        &state,
        UserView {
            id,
            username,
            display_name,
        },
        StatusCode::OK,
    )
    .await
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Response> {
    if let Some(token) = cookie_value(&headers, SESSION_COOKIE) {
        sqlx::query("DELETE FROM auth_sessions WHERE token_hash = ?")
            .bind(token_hash(&token))
            .execute(&state.db)
            .await?;
    }
    Ok((
        StatusCode::NO_CONTENT,
        [(
            header::SET_COOKIE,
            format!("{SESSION_COOKIE}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0"),
        )],
    )
        .into_response())
}

async fn me(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<UserView>> {
    authenticated_user(&state, &headers)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::Unauthorized("请先登录".to_owned()))
}

async fn create_session_response(
    state: &AppState,
    user: UserView,
    status: StatusCode,
) -> AppResult<Response> {
    let mut bytes = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let token = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let expires_at =
        (Utc::now() + Duration::days(SESSION_DAYS)).to_rfc3339_opts(SecondsFormat::Millis, true);
    sqlx::query(
        "INSERT INTO auth_sessions (id, user_id, token_hash, expires_at) VALUES (?, ?, ?, ?)",
    )
    .bind(Uuid::now_v7().to_string())
    .bind(&user.id)
    .bind(token_hash(&token))
    .bind(expires_at)
    .execute(&state.db)
    .await?;
    let cookie = format!(
        "{SESSION_COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}",
        SESSION_DAYS * 24 * 60 * 60
    );
    Ok((
        status,
        [(header::SET_COOKIE, cookie)],
        Json(AuthResponse { user }),
    )
        .into_response())
}

async fn authenticated_user(state: &AppState, headers: &HeaderMap) -> AppResult<Option<UserView>> {
    let Some(token) = cookie_value(headers, SESSION_COOKIE) else {
        return Ok(None);
    };
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let user = sqlx::query_as::<_, UserView>(
        r#"SELECT u.id, u.username, u.display_name
           FROM auth_sessions s JOIN users u ON u.id = s.user_id
           WHERE s.token_hash = ? AND s.expires_at > ?"#,
    )
    .bind(token_hash(&token))
    .bind(&now)
    .fetch_optional(&state.db)
    .await?;
    if user.is_some() {
        sqlx::query("UPDATE auth_sessions SET last_seen_at = ? WHERE token_hash = ?")
            .bind(now)
            .bind(token_hash(&token))
            .execute(&state.db)
            .await?;
    }
    Ok(user)
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|part| part.trim().split_once('='))
        .find_map(|(key, value)| (key == name).then(|| value.to_owned()))
}

fn token_hash(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn validate_username(value: &str) -> AppResult<String> {
    let value = common::required_text(value, "用户名", 64)?.to_ascii_lowercase();
    if value.len() < 3
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
    {
        return Err(AppError::Validation(
            "用户名至少 3 位，只能包含字母、数字、点、横线和下划线".to_owned(),
        ));
    }
    Ok(value)
}

fn validate_password(value: &str) -> AppResult<()> {
    if value.chars().count() < 10 || value.chars().count() > 128 {
        return Err(AppError::Validation(
            "密码长度必须为 10 至 128 位".to_owned(),
        ));
    }
    if !value
        .chars()
        .any(|character| character.is_ascii_alphabetic())
        || !value.chars().any(|character| character.is_ascii_digit())
    {
        return Err(AppError::Validation(
            "密码必须同时包含字母和数字".to_owned(),
        ));
    }
    Ok(())
}
