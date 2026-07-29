pub mod api;
pub mod config;
pub mod db;
pub mod domain;
pub mod error;
pub mod models;

use axum::Router;
use sqlx::SqlitePool;

pub fn build_app(pool: SqlitePool) -> Router {
    build_app_with_auth(pool, true)
}

pub fn build_app_with_auth(pool: SqlitePool, auth_required: bool) -> Router {
    build_service_app(pool, auth_required, None)
}

pub fn build_service_app(
    pool: SqlitePool,
    auth_required: bool,
    internal_api_token: Option<String>,
) -> Router {
    api::router(
        api::AppState {
            db: pool,
            auth_required,
        },
        internal_api_token,
    )
}
