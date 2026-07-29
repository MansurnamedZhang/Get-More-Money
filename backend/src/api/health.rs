use super::AppState;
use crate::error::AppResult;
use axum::{Json, extract::State};
use serde::Serialize;

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
    database: &'static str,
    version: &'static str,
}

pub async fn health(State(state): State<AppState>) -> AppResult<Json<HealthResponse>> {
    sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.db)
        .await?;

    Ok(Json(HealthResponse {
        status: "ok",
        database: "connected",
        version: env!("CARGO_PKG_VERSION"),
    }))
}
