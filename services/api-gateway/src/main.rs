use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::{any, get},
};
use investment_contracts::{
    ActorContext, HEADER_ACTOR_ID, HEADER_ACTOR_NAME, HEADER_CORRELATION_ID, HEADER_INTERNAL_TOKEN,
    ServiceTarget, classify_api_path,
};
use reqwest::{Client, redirect::Policy};
use serde::Serialize;
use std::{env, net::SocketAddr, sync::Arc, time::Duration};
use thiserror::Error;
use tokio::net::TcpListener;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

const MAX_REQUEST_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone)]
struct AppState {
    client: Client,
    config: Arc<GatewayConfig>,
}

#[derive(Debug)]
struct GatewayConfig {
    bind_addr: SocketAddr,
    identity_url: String,
    investment_core_url: String,
    market_data_url: String,
    planning_url: String,
    audit_url: String,
    internal_token: String,
}

impl GatewayConfig {
    fn from_env() -> Result<Self, String> {
        let bind_addr = env_value("GATEWAY_BIND", "127.0.0.1:3001")
            .parse::<SocketAddr>()
            .map_err(|error| format!("GATEWAY_BIND 无效：{error}"))?;
        let internal_token = env_value("INTERNAL_API_TOKEN", "local-dev-change-me");
        if !bind_addr.ip().is_loopback() && internal_token == "local-dev-change-me" {
            return Err("非本地环境必须配置独立的 INTERNAL_API_TOKEN".to_owned());
        }
        Ok(Self {
            bind_addr,
            identity_url: normalized_url(&env_value(
                "IDENTITY_SERVICE_URL",
                "http://127.0.0.1:3101",
            )),
            investment_core_url: normalized_url(&env_value(
                "INVESTMENT_CORE_SERVICE_URL",
                "http://127.0.0.1:3100",
            )),
            market_data_url: normalized_url(&env_value(
                "MARKET_DATA_SERVICE_URL",
                "http://127.0.0.1:3100",
            )),
            planning_url: normalized_url(&env_value(
                "PLANNING_SERVICE_URL",
                "http://127.0.0.1:3100",
            )),
            audit_url: normalized_url(&env_value("AUDIT_SERVICE_URL", "http://127.0.0.1:3100")),
            internal_token,
        })
    }

    fn upstream(&self, target: ServiceTarget) -> &str {
        match target {
            ServiceTarget::Identity => &self.identity_url,
            ServiceTarget::InvestmentCore => &self.investment_core_url,
            ServiceTarget::MarketData => &self.market_data_url,
            ServiceTarget::Planning => &self.planning_url,
            ServiceTarget::Audit => &self.audit_url,
        }
    }
}

#[derive(Debug, Error)]
enum GatewayError {
    #[error("请求体过大或无法读取")]
    InvalidBody,
    #[error("身份服务暂时不可用")]
    IdentityUnavailable,
    #[error("请先登录")]
    Unauthorized,
    #[error("上游服务暂时不可用：{0:?}")]
    UpstreamUnavailable(ServiceTarget),
    #[error("无法构建网关响应")]
    InvalidResponse,
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

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::InvalidBody => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "invalid_request_body",
                self.to_string(),
            ),
            Self::IdentityUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "identity_unavailable",
                self.to_string(),
            ),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized", self.to_string()),
            Self::UpstreamUnavailable(_) => (
                StatusCode::BAD_GATEWAY,
                "upstream_unavailable",
                self.to_string(),
            ),
            Self::InvalidResponse => (
                StatusCode::BAD_GATEWAY,
                "invalid_upstream_response",
                self.to_string(),
            ),
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

type GatewayResult<T> = Result<T, GatewayError>;

#[derive(Serialize)]
struct HealthResponse {
    service: &'static str,
    status: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct ReadinessResponse {
    status: &'static str,
    dependencies: Vec<DependencyStatus>,
}

#[derive(Serialize)]
struct DependencyStatus {
    service: &'static str,
    ready: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "api_gateway=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Arc::new(GatewayConfig::from_env()?);
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(45))
        .redirect(Policy::none())
        .build()?;
    let state = AppState { client, config };

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

    let app = Router::new()
        .route("/health", get(health))
        .route("/ready", get(readiness))
        .route("/api/v1/health", get(health))
        .fallback(any(proxy))
        .with_state(state.clone())
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(state.config.bind_addr).await?;
    info!(address = %state.config.bind_addr, "api gateway listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        service: "api_gateway",
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn readiness(State(state): State<AppState>) -> (StatusCode, Json<ReadinessResponse>) {
    let identity = check_health(&state.client, &state.config.identity_url, "/health");
    let core = check_health(&state.client, &state.config.investment_core_url, "/health");
    let market = check_health(&state.client, &state.config.market_data_url, "/health");
    let planning = check_health(&state.client, &state.config.planning_url, "/health");
    let audit = check_health(&state.client, &state.config.audit_url, "/health");
    let (identity_ready, core_ready, market_ready, planning_ready, audit_ready) =
        tokio::join!(identity, core, market, planning, audit);
    let ready = identity_ready && core_ready && market_ready && planning_ready && audit_ready;
    (
        if ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        Json(ReadinessResponse {
            status: if ready { "ready" } else { "degraded" },
            dependencies: vec![
                DependencyStatus {
                    service: "identity",
                    ready: identity_ready,
                },
                DependencyStatus {
                    service: "investment_core",
                    ready: core_ready,
                },
                DependencyStatus {
                    service: "market_data",
                    ready: market_ready,
                },
                DependencyStatus {
                    service: "planning",
                    ready: planning_ready,
                },
                DependencyStatus {
                    service: "audit",
                    ready: audit_ready,
                },
            ],
        }),
    )
}

async fn proxy(State(state): State<AppState>, request: Request) -> GatewayResult<Response> {
    let (parts, body) = request.into_parts();
    let target = classify_api_path(parts.uri.path());
    let correlation_id = correlation_id(&parts.headers);
    let actor = if target == ServiceTarget::Identity {
        None
    } else {
        Some(validate_session(&state, &parts.headers, correlation_id).await?)
    };
    let body = to_bytes(body, MAX_REQUEST_BYTES)
        .await
        .map_err(|_| GatewayError::InvalidBody)?;
    let upstream = state.config.upstream(target);
    let url = upstream_url(upstream, &parts.uri);
    let mut outbound = state.client.request(parts.method.clone(), url);

    for (name, value) in &parts.headers {
        if should_forward_request_header(name) {
            outbound = outbound.header(name, value);
        }
    }
    outbound = outbound
        .header(HEADER_CORRELATION_ID, correlation_id.to_string())
        .header(HEADER_INTERNAL_TOKEN, &state.config.internal_token);
    if let Some(actor) = actor {
        outbound = outbound
            .header(HEADER_ACTOR_ID, actor.id)
            .header(HEADER_ACTOR_NAME, actor.username);
    }

    let upstream_response = outbound.body(body).send().await.map_err(|error| {
        warn!(?error, ?target, "upstream request failed");
        GatewayError::UpstreamUnavailable(target)
    })?;
    let status = upstream_response.status();
    let headers = upstream_response.headers().clone();
    let bytes = upstream_response.bytes().await.map_err(|error| {
        warn!(?error, ?target, "upstream response body failed");
        GatewayError::InvalidResponse
    })?;

    let mut builder = Response::builder().status(status);
    let response_headers = builder.headers_mut().ok_or(GatewayError::InvalidResponse)?;
    for (name, value) in &headers {
        if should_forward_response_header(name) {
            response_headers.append(name, value.clone());
        }
    }
    response_headers.insert(
        HeaderName::from_static(HEADER_CORRELATION_ID),
        HeaderValue::from_str(&correlation_id.to_string())
            .map_err(|_| GatewayError::InvalidResponse)?,
    );
    builder
        .body(Body::from(bytes))
        .map_err(|_| GatewayError::InvalidResponse)
}

async fn validate_session(
    state: &AppState,
    headers: &HeaderMap,
    correlation_id: Uuid,
) -> GatewayResult<ActorContext> {
    let url = format!("{}/internal/auth/validate", state.config.identity_url);
    let mut request = state
        .client
        .get(url)
        .header(HEADER_INTERNAL_TOKEN, &state.config.internal_token)
        .header(HEADER_CORRELATION_ID, correlation_id.to_string());
    if let Some(cookie) = headers.get(header::COOKIE) {
        request = request.header(header::COOKIE, cookie);
    }
    let response = request
        .send()
        .await
        .map_err(|_| GatewayError::IdentityUnavailable)?;
    if response.status() == StatusCode::UNAUTHORIZED {
        return Err(GatewayError::Unauthorized);
    }
    if !response.status().is_success() {
        return Err(GatewayError::IdentityUnavailable);
    }
    response
        .json::<ActorContext>()
        .await
        .map_err(|_| GatewayError::IdentityUnavailable)
}

async fn check_health(client: &Client, base: &str, path: &str) -> bool {
    client
        .get(format!("{base}{path}"))
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .is_ok_and(|response| response.status().is_success())
}

fn correlation_id(headers: &HeaderMap) -> Uuid {
    headers
        .get(HEADER_CORRELATION_ID)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value).ok())
        .unwrap_or_else(Uuid::now_v7)
}

fn upstream_url(base: &str, uri: &Uri) -> String {
    let query = uri
        .query()
        .map(|value| format!("?{value}"))
        .unwrap_or_default();
    format!("{}{path}{query}", base, path = uri.path())
}

fn should_forward_request_header(name: &HeaderName) -> bool {
    !matches!(
        name.as_str(),
        "host"
            | "content-length"
            | "connection"
            | "transfer-encoding"
            | HEADER_INTERNAL_TOKEN
            | HEADER_ACTOR_ID
            | HEADER_ACTOR_NAME
    )
}

fn should_forward_response_header(name: &HeaderName) -> bool {
    !matches!(
        name.as_str(),
        "content-length"
            | "connection"
            | "transfer-encoding"
            | "access-control-allow-origin"
            | "access-control-allow-credentials"
    )
}

fn normalized_url(value: &str) -> String {
    value.trim_end_matches('/').to_owned()
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
