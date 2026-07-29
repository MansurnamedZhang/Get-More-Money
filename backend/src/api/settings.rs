use super::{AppState, common};
use crate::error::{AppError, AppResult};
use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use reqwest::{Client, Proxy, redirect::Policy};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::time::{Duration, Instant};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AppSettings {
    pub report_currency: String,
    pub timezone: String,
    pub cost_method: String,
    pub stale_price_days: i64,
    pub absolute_rebalance_threshold: String,
    pub relative_rebalance_threshold: String,
    pub transaction_hard_delete_minutes: i64,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
struct SettingsInput {
    report_currency: String,
    timezone: String,
    cost_method: String,
    stale_price_days: i64,
    absolute_rebalance_threshold: String,
    relative_rebalance_threshold: String,
    transaction_hard_delete_minutes: i64,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct NetworkProxySettings {
    pub is_enabled: bool,
    pub protocol: String,
    pub host: String,
    pub port: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct NetworkProxyInput {
    #[serde(default)]
    is_enabled: bool,
    protocol: String,
    host: String,
    port: i64,
}

#[derive(Debug, Serialize)]
struct NetworkProxyTestResult {
    ok: bool,
    mode: String,
    target: &'static str,
    status: u16,
    latency_ms: u128,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/settings", get(get_settings).put(update_settings))
        .route(
            "/network-proxy",
            get(get_network_proxy).put(update_network_proxy),
        )
        .route("/network-proxy/test", post(test_network_proxy))
}

pub async fn load(state: &AppState) -> AppResult<AppSettings> {
    Ok(sqlx::query_as::<_, AppSettings>(
        r#"SELECT report_currency, timezone, cost_method, stale_price_days,
                  absolute_rebalance_threshold, relative_rebalance_threshold,
                  transaction_hard_delete_minutes, updated_at
           FROM app_settings WHERE id = 1"#,
    )
    .fetch_one(&state.db)
    .await?)
}

async fn get_settings(State(state): State<AppState>) -> AppResult<Json<AppSettings>> {
    Ok(Json(load(&state).await?))
}

async fn update_settings(
    State(state): State<AppState>,
    Json(input): Json<SettingsInput>,
) -> AppResult<Json<AppSettings>> {
    let currency = common::major_currency(&input.report_currency, "报告币种")?;
    let timezone = common::required_text(&input.timezone, "时区", 80)?;
    if !matches!(input.cost_method.as_str(), "average" | "fifo") {
        return Err(AppError::Validation(
            "成本法必须是 average 或 fifo".to_owned(),
        ));
    }
    if !(0..=365).contains(&input.stale_price_days) {
        return Err(AppError::Validation(
            "价格陈旧天数必须在 0 到 365 之间".to_owned(),
        ));
    }
    if !(0..=10_080).contains(&input.transaction_hard_delete_minutes) {
        return Err(AppError::Validation(
            "流水彻底删除时限必须在 0 到 10080 分钟之间".to_owned(),
        ));
    }
    let absolute = weight(&input.absolute_rebalance_threshold, "绝对再平衡阈值")?;
    let relative = weight(&input.relative_rebalance_threshold, "相对再平衡阈值")?;
    sqlx::query(
        r#"UPDATE app_settings SET report_currency = ?, timezone = ?, cost_method = ?,
           stale_price_days = ?, absolute_rebalance_threshold = ?, relative_rebalance_threshold = ?,
           transaction_hard_delete_minutes = ?,
           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = 1"#,
    )
    .bind(currency)
    .bind(timezone)
    .bind(input.cost_method)
    .bind(input.stale_price_days)
    .bind(absolute)
    .bind(relative)
    .bind(input.transaction_hard_delete_minutes)
    .execute(&state.db)
    .await?;
    Ok(Json(load(&state).await?))
}

pub async fn load_network_proxy(pool: &SqlitePool) -> AppResult<NetworkProxySettings> {
    Ok(sqlx::query_as::<_, NetworkProxySettings>(
        r#"SELECT is_enabled, protocol, host, port, updated_at
           FROM network_proxy_settings WHERE id = 1"#,
    )
    .fetch_one(pool)
    .await?)
}

async fn get_network_proxy(State(state): State<AppState>) -> AppResult<Json<NetworkProxySettings>> {
    Ok(Json(load_network_proxy(&state.db).await?))
}

async fn update_network_proxy(
    State(state): State<AppState>,
    Json(input): Json<NetworkProxyInput>,
) -> AppResult<Json<NetworkProxySettings>> {
    let values = validate_network_proxy(input)?;
    sqlx::query(
        r#"UPDATE network_proxy_settings
           SET is_enabled = ?, protocol = ?, host = ?, port = ?,
               updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
           WHERE id = 1"#,
    )
    .bind(values.is_enabled)
    .bind(values.protocol)
    .bind(values.host)
    .bind(values.port)
    .execute(&state.db)
    .await?;
    Ok(Json(load_network_proxy(&state.db).await?))
}

async fn test_network_proxy(
    Json(input): Json<NetworkProxyInput>,
) -> AppResult<Json<NetworkProxyTestResult>> {
    const TARGET: &str = "https://api.frankfurter.dev/v2/rate/USD/CNY";
    let settings = validate_network_proxy(input)?;
    let client = build_http_client(&settings, Duration::from_secs(12))?;
    let started = Instant::now();
    let response = client
        .get(TARGET)
        .header("user-agent", "SANYU-Invest/0.1")
        .send()
        .await
        .map_err(|error| AppError::External(format!("代理连通测试失败：{error}")))?;
    let status = response.status();
    if !status.is_success() {
        return Err(AppError::External(format!("测试服务返回状态 {}", status)));
    }
    Ok(Json(NetworkProxyTestResult {
        ok: true,
        mode: if settings.is_enabled {
            format!(
                "{}://{}:{}",
                settings.protocol, settings.host, settings.port
            )
        } else {
            "direct".to_owned()
        },
        target: TARGET,
        status: status.as_u16(),
        latency_ms: started.elapsed().as_millis(),
    }))
}

pub async fn http_client(pool: &SqlitePool, timeout: Duration) -> AppResult<Client> {
    let settings = load_network_proxy(pool).await?;
    build_http_client(&settings, timeout)
}

fn build_http_client(settings: &NetworkProxySettings, timeout: Duration) -> AppResult<Client> {
    let mut builder = Client::builder()
        .timeout(timeout)
        .redirect(Policy::none())
        .no_proxy();
    if settings.is_enabled {
        let scheme = if settings.protocol == "socks5" {
            "socks5h"
        } else {
            settings.protocol.as_str()
        };
        let proxy_url = format!("{scheme}://{}:{}", settings.host, settings.port);
        let proxy = Proxy::all(&proxy_url)
            .map_err(|error| AppError::Validation(format!("代理地址无效：{error}")))?;
        builder = builder.proxy(proxy);
    }
    builder
        .build()
        .map_err(|error| AppError::External(format!("网络客户端创建失败：{error}")))
}

fn validate_network_proxy(input: NetworkProxyInput) -> AppResult<NetworkProxySettings> {
    if !matches!(input.protocol.as_str(), "http" | "https" | "socks5") {
        return Err(AppError::Validation(
            "代理协议必须是 HTTP、HTTPS 或 SOCKS5".to_owned(),
        ));
    }
    let host = input.host.trim();
    if host.is_empty() || host.len() > 253 {
        return Err(AppError::Validation("代理主机不能为空".to_owned()));
    }
    if !host
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'))
    {
        return Err(AppError::Validation(
            "代理主机只能填写域名、localhost 或 IPv4 地址，不要包含协议和路径".to_owned(),
        ));
    }
    if !(1..=65_535).contains(&input.port) {
        return Err(AppError::Validation(
            "代理端口必须在 1 到 65535 之间".to_owned(),
        ));
    }
    Ok(NetworkProxySettings {
        is_enabled: input.is_enabled,
        protocol: input.protocol,
        host: host.to_owned(),
        port: input.port,
        updated_at: String::new(),
    })
}

pub fn weight(value: &str, field: &str) -> AppResult<String> {
    use rust_decimal::Decimal;
    use std::str::FromStr;
    let normalized = common::positive_decimal(value, field)?;
    let parsed = Decimal::from_str(&normalized).expect("validated decimal");
    if parsed > Decimal::ONE {
        return Err(AppError::Validation(format!("{field} 不能大于 1")));
    }
    Ok(normalized)
}
