use super::{AppState, common};
use crate::{
    error::{AppError, AppResult},
    models::BlockchainNetwork,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct NetworkInput {
    code: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct NetworkStatusInput {
    is_active: bool,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/blockchain-networks",
            get(list_networks).post(create_network),
        )
        .route(
            "/blockchain-networks/{id}",
            get(get_network)
                .put(replace_network)
                .patch(set_network_status)
                .delete(delete_network),
        )
}

async fn list_networks(State(state): State<AppState>) -> AppResult<Json<Vec<BlockchainNetwork>>> {
    let networks = sqlx::query_as::<_, BlockchainNetwork>(
        "SELECT * FROM blockchain_networks ORDER BY is_active DESC, lower(name), lower(code)",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(networks))
}

async fn get_network(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<BlockchainNetwork>> {
    let id = common::id(&id, "区块链网络 ID")?;
    Ok(Json(fetch_network(&state, &id).await?))
}

async fn create_network(
    State(state): State<AppState>,
    Json(input): Json<NetworkInput>,
) -> AppResult<(StatusCode, Json<BlockchainNetwork>)> {
    let (code, name) = validate(input)?;
    let id = Uuid::now_v7().to_string();
    let result = sqlx::query("INSERT INTO blockchain_networks (id, code, name) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(code)
        .bind(name)
        .execute(&state.db)
        .await;
    if let Err(error) = result {
        if common::is_unique_violation(&error) {
            return Err(AppError::Conflict("相同代码的区块链网络已存在".to_owned()));
        }
        return Err(error.into());
    }
    Ok((StatusCode::CREATED, Json(fetch_network(&state, &id).await?)))
}

async fn replace_network(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<NetworkInput>,
) -> AppResult<Json<BlockchainNetwork>> {
    let id = common::id(&id, "区块链网络 ID")?;
    let current = fetch_network(&state, &id).await?;
    let (code, name) = validate(input)?;
    if !current.code.eq_ignore_ascii_case(&code) && network_is_used(&state, &current.code).await? {
        return Err(AppError::Conflict(
            "该网络已被投资标的使用，不能修改代码；可以修改显示名称".to_owned(),
        ));
    }
    let result = sqlx::query(
        r#"UPDATE blockchain_networks
           SET code = ?, name = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
           WHERE id = ?"#,
    )
    .bind(code)
    .bind(name)
    .bind(&id)
    .execute(&state.db)
    .await;
    match result {
        Ok(_) => Ok(Json(fetch_network(&state, &id).await?)),
        Err(error) if common::is_unique_violation(&error) => {
            Err(AppError::Conflict("相同代码的区块链网络已存在".to_owned()))
        }
        Err(error) => Err(error.into()),
    }
}

async fn set_network_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<NetworkStatusInput>,
) -> AppResult<Json<BlockchainNetwork>> {
    let id = common::id(&id, "区块链网络 ID")?;
    let result = sqlx::query(
        r#"UPDATE blockchain_networks
           SET is_active = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
           WHERE id = ?"#,
    )
    .bind(input.is_active)
    .bind(&id)
    .execute(&state.db)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("区块链网络不存在".to_owned()));
    }
    Ok(Json(fetch_network(&state, &id).await?))
}

async fn delete_network(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let id = common::id(&id, "区块链网络 ID")?;
    let current = fetch_network(&state, &id).await?;
    if network_is_used(&state, &current.code).await? {
        return Err(AppError::Conflict(
            "该网络已被投资标的使用，不能删除；请先从相关标的中移除或停用网络".to_owned(),
        ));
    }
    sqlx::query("DELETE FROM blockchain_networks WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn validate(input: NetworkInput) -> AppResult<(String, String)> {
    let code = common::required_text(&input.code, "网络代码", 32)?.to_ascii_lowercase();
    if !code
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(AppError::Validation(
            "网络代码只能包含字母、数字、连字符或下划线".to_owned(),
        ));
    }
    let name = common::required_text(&input.name, "网络名称", 80)?;
    Ok((code, name))
}

async fn network_is_used(state: &AppState, code: &str) -> AppResult<bool> {
    let needle = format!(",{},", code.to_ascii_lowercase());
    Ok(sqlx::query_scalar(
        r#"SELECT EXISTS(
             SELECT 1 FROM instruments
             WHERE instr(',' || lower(replace(COALESCE(network, ''), ' ', '')) || ',', ?) > 0
           )"#,
    )
    .bind(needle)
    .fetch_one(&state.db)
    .await?)
}

async fn fetch_network(state: &AppState, id: &str) -> AppResult<BlockchainNetwork> {
    sqlx::query_as::<_, BlockchainNetwork>("SELECT * FROM blockchain_networks WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("区块链网络不存在".to_owned()))
}
