use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::{Duration, SecondsFormat, Utc};
use investment_contracts::{HEADER_INTERNAL_TOKEN, ServiceTarget};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{
    SqlitePool,
    migrate::MigrateError,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use std::{
    env, net::SocketAddr, path::Path, str::FromStr, sync::Arc, time::Duration as StdDuration,
};
use thiserror::Error;
use tokio::net::TcpListener;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

const SESSION_COOKIE: &str = "investment_session";
const SESSION_DAYS: i64 = 30;
const MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
    internal_token: Arc<str>,
    secure_cookie: bool,
}

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

#[derive(Debug, Serialize)]
struct HealthResponse {
    service: ServiceTarget,
    status: &'static str,
    version: &'static str,
}

#[derive(Debug, Error)]
enum AppError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Validation(message) => (StatusCode::BAD_REQUEST, "validation_error", message),
            Self::Conflict(message) => (StatusCode::CONFLICT, "conflict", message),
            Self::Unauthorized(message) => (StatusCode::UNAUTHORIZED, "unauthorized", message),
            Self::Forbidden(message) => (StatusCode::FORBIDDEN, "forbidden", message),
            Self::Database(error) => {
                tracing::error!(?error, "identity database request failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database_error",
                    "身份数据暂时不可用".to_owned(),
                )
            }
        };
        (
            status,
            Json(ErrorEnvelope {
                error: ErrorBody { code, message },
            }),
        )
            .into_response()
    }
}

type AppResult<T> = Result<T, AppError>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "identity_service=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let bind_addr = env_value("IDENTITY_BIND", "127.0.0.1:3101")
        .parse::<SocketAddr>()
        .map_err(|error| format!("IDENTITY_BIND 无效：{error}"))?;
    let database_url = env_value(
        "IDENTITY_DATABASE_URL",
        "sqlite://data/services/identity.db",
    );
    let internal_token = env_value("INTERNAL_API_TOKEN", "local-dev-change-me");
    if !bind_addr.ip().is_loopback() && internal_token == "local-dev-change-me" {
        return Err("非本地环境必须配置独立的 INTERNAL_API_TOKEN".into());
    }
    let secure_cookie = env_value("COOKIE_SECURE", "false")
        .parse::<bool>()
        .map_err(|error| format!("COOKIE_SECURE 无效：{error}"))?;
    let db = connect(&database_url).await?;
    let state = AppState {
        db,
        internal_token: Arc::from(internal_token),
        secure_cookie,
    };

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list([
            HeaderValue::from_static("http://localhost:3000"),
            HeaderValue::from_static("http://127.0.0.1:3000"),
        ]))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE])
        .allow_credentials(true);

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/v1/auth/status", get(status))
        .route("/api/v1/auth/setup", post(setup))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/me", get(me))
        .route("/internal/auth/validate", get(validate_internal))
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(bind_addr).await?;
    info!(address = %bind_addr, "identity service listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        service: ServiceTarget::Identity,
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn status(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<AuthStatus>> {
    cleanup_expired_sessions(&state.db).await?;
    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await?;
    let user = authenticated_user(&state.db, &headers, true).await?;
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
    let display_name = required_text(&input.display_name, "显示名称", 80)?;
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
    authenticated_user(&state.db, &headers, true)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::Unauthorized("请先登录".to_owned()))
}

async fn validate_internal(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<UserView>> {
    let supplied = headers
        .get(HEADER_INTERNAL_TOKEN)
        .and_then(|value| value.to_str().ok());
    if supplied != Some(state.internal_token.as_ref()) {
        return Err(AppError::Forbidden("内部服务凭据无效".to_owned()));
    }
    authenticated_user(&state.db, &headers, false)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::Unauthorized("会话已失效，请重新登录".to_owned()))
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

    let secure = if state.secure_cookie { "; Secure" } else { "" };
    let cookie = format!(
        "{SESSION_COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}{}",
        SESSION_DAYS * 24 * 60 * 60,
        secure
    );
    Ok((
        status,
        [(header::SET_COOKIE, cookie)],
        Json(AuthResponse { user }),
    )
        .into_response())
}

async fn authenticated_user(
    db: &SqlitePool,
    headers: &HeaderMap,
    touch: bool,
) -> AppResult<Option<UserView>> {
    let Some(token) = cookie_value(headers, SESSION_COOKIE) else {
        return Ok(None);
    };
    let hash = token_hash(&token);
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let user = sqlx::query_as::<_, UserView>(
        r#"SELECT u.id, u.username, u.display_name
           FROM auth_sessions s
           JOIN users u ON u.id = s.user_id
           WHERE s.token_hash = ? AND s.expires_at > ?"#,
    )
    .bind(&hash)
    .bind(&now)
    .fetch_optional(db)
    .await?;
    if touch && user.is_some() {
        sqlx::query("UPDATE auth_sessions SET last_seen_at = ? WHERE token_hash = ?")
            .bind(now)
            .bind(hash)
            .execute(db)
            .await?;
    }
    Ok(user)
}

async fn cleanup_expired_sessions(db: &SqlitePool) -> AppResult<()> {
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    sqlx::query("DELETE FROM auth_sessions WHERE expires_at <= ?")
        .bind(now)
        .execute(db)
        .await?;
    Ok(())
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
    let value = required_text(value, "用户名", 64)?.to_ascii_lowercase();
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
    let length = value.chars().count();
    if !(10..=128).contains(&length) {
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

fn required_text(value: &str, label: &str, max: usize) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::Validation(format!("{label}不能为空")));
    }
    if value.chars().count() > max {
        return Err(AppError::Validation(format!(
            "{label}不能超过 {max} 个字符"
        )));
    }
    Ok(value.to_owned())
}

async fn connect(database_url: &str) -> Result<SqlitePool, DbInitError> {
    ensure_database_directory(database_url)?;
    let is_memory = database_url.contains(":memory:");
    let mut options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .busy_timeout(StdDuration::from_secs(5));
    if !is_memory {
        options = options.journal_mode(SqliteJournalMode::Wal);
    }
    let pool = SqlitePoolOptions::new()
        .max_connections(if is_memory { 1 } else { 5 })
        .connect_with(options)
        .await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

#[derive(Debug, Error)]
enum DbInitError {
    #[error("failed to prepare identity database directory: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to connect identity database: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("failed to migrate identity database: {0}")]
    Migrate(#[from] MigrateError),
}

fn ensure_database_directory(database_url: &str) -> Result<(), std::io::Error> {
    if database_url.contains(":memory:") {
        return Ok(());
    }
    if let Some(raw_path) = database_url.strip_prefix("sqlite://") {
        let raw_path = raw_path.split('?').next().unwrap_or(raw_path);
        if let Some(parent) = Path::new(raw_path).parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

fn env_value(key: &str, fallback: &str) -> String {
    env::var(key).unwrap_or_else(|_| fallback.to_owned())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
