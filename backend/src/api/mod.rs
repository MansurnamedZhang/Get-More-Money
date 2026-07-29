mod accounts;
mod audit_exports;
pub mod auth;
mod common;
mod health;
mod imports;
mod instruments;
mod maintenance;
pub mod market;
mod networks;
mod planning;
mod portfolio;
mod settings;
pub mod sync;
mod transactions;

use axum::{
    Router,
    extract::{Request, State},
    http::{HeaderValue, Method, header},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::get,
};
use investment_contracts::HEADER_INTERNAL_TOKEN;
use sqlx::SqlitePool;
use subtle::ConstantTimeEq;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub auth_required: bool,
}

pub fn router(state: AppState, internal_api_token: Option<String>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list([
            HeaderValue::from_static("http://localhost:3000"),
            HeaderValue::from_static("http://127.0.0.1:3000"),
        ]))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_headers([header::CONTENT_TYPE])
        .expose_headers([header::CONTENT_DISPOSITION])
        .allow_credentials(true);

    let protected = Router::new()
        .merge(accounts::routes())
        .merge(audit_exports::routes())
        .merge(instruments::routes())
        .merge(imports::routes())
        .merge(market::routes())
        .merge(maintenance::routes())
        .merge(networks::routes())
        .merge(planning::routes())
        .merge(portfolio::routes())
        .merge(settings::routes())
        .merge(sync::routes())
        .merge(transactions::routes())
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::require_auth,
        ));

    let private_api = Router::new().merge(auth::routes()).merge(protected);
    let private_api = if let Some(token) = internal_api_token {
        private_api.route_layer(axum::middleware::from_fn_with_state(
            token,
            require_internal_gateway,
        ))
    } else {
        private_api
    };

    let api = Router::new()
        .route("/health", get(health::health))
        .merge(private_api);

    Router::new()
        .route("/health", get(health::health))
        .nest("/api/v1", api)
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}

async fn require_internal_gateway(
    State(expected_token): State<String>,
    request: Request,
    next: Next,
) -> Response {
    let valid = request
        .headers()
        .get(HEADER_INTERNAL_TOKEN)
        .is_some_and(|provided| provided.as_bytes().ct_eq(expected_token.as_bytes()).into());
    if valid {
        next.run(request).await
    } else {
        crate::error::AppError::Unauthorized("请求未通过内部网关验证".to_owned()).into_response()
    }
}
