use super::{AppState, common};
use crate::error::{AppError, AppResult};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use chrono::{Duration, SecondsFormat, Utc};
use reqwest::{
    Url,
    header::{HeaderName, HeaderValue},
};
use ring::{
    aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey},
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    net::IpAddr,
    path::PathBuf,
    sync::OnceLock,
    time::Instant,
};
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
struct DataSourceRow {
    id: String,
    name: String,
    source_type: String,
    priority: i64,
    credentials_ref: Option<String>,
    config_json: String,
    is_enabled: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
struct DataSourceView {
    id: String,
    name: String,
    source_type: String,
    priority: i64,
    credentials_ref: Option<String>,
    config: Value,
    is_enabled: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct DataSourceInput {
    name: String,
    source_type: String,
    #[serde(default = "default_priority")]
    priority: i64,
    credentials_ref: Option<String>,
    #[serde(default)]
    config: Value,
    #[serde(default)]
    is_enabled: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct SyncJob {
    id: String,
    data_source_id: String,
    name: String,
    data_type: String,
    interval_seconds: i64,
    timezone: String,
    cursor: Option<String>,
    retry_policy_json: String,
    next_run_at: Option<String>,
    last_run_at: Option<String>,
    is_enabled: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct SyncJobInput {
    data_source_id: String,
    name: String,
    data_type: String,
    interval_seconds: i64,
    #[serde(default = "default_timezone")]
    timezone: String,
    #[serde(default)]
    retry_policy: Value,
    #[serde(default)]
    is_enabled: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct SyncRun {
    id: String,
    job_id: String,
    started_at: String,
    finished_at: Option<String>,
    status: String,
    stats_json: String,
    error_message: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct JobSource {
    job_id: String,
    data_source_id: String,
    data_type: String,
    interval_seconds: i64,
    source_name: String,
    config_json: String,
}

#[derive(Debug, sqlx::FromRow)]
struct StockInstrumentRow {
    id: String,
    symbol: String,
    currency: String,
}

#[derive(Debug, sqlx::FromRow)]
struct ApiCollectorRow {
    id: String,
    source_id: String,
    name: String,
    source_type: String,
    priority: i64,
    config_json: String,
    data_type: String,
    interval_seconds: i64,
    timezone: String,
    next_run_at: Option<String>,
    last_run_at: Option<String>,
    is_enabled: bool,
    created_at: String,
    updated_at: String,
    latest_run_status: Option<String>,
    latest_run_at: Option<String>,
    latest_error: Option<String>,
    has_api_key: bool,
}

#[derive(Debug, Serialize)]
struct ApiCollectorView {
    id: String,
    source_id: String,
    name: String,
    source_type: String,
    priority: i64,
    config: Value,
    data_type: String,
    interval_seconds: i64,
    timezone: String,
    next_run_at: Option<String>,
    last_run_at: Option<String>,
    is_enabled: bool,
    created_at: String,
    updated_at: String,
    latest_run_status: Option<String>,
    latest_run_at: Option<String>,
    latest_error: Option<String>,
    has_api_key: bool,
}

#[derive(Debug, Deserialize)]
struct ApiCollectorInput {
    name: String,
    source_type: String,
    #[serde(default = "default_priority")]
    priority: i64,
    #[serde(default)]
    config: Value,
    data_type: String,
    interval_seconds: i64,
    #[serde(default = "default_timezone")]
    timezone: String,
    #[serde(default)]
    is_enabled: bool,
    api_key: Option<String>,
    #[serde(default)]
    clear_api_key: bool,
}

#[derive(Debug, Deserialize)]
struct ApiCollectorTestInput {
    collector_id: Option<String>,
    name: Option<String>,
    data_type: String,
    #[serde(default)]
    config: Value,
    api_key: Option<String>,
}

#[derive(Debug, Serialize)]
struct ApiCollectorTestResult {
    success: bool,
    provider: String,
    data_type: String,
    request_url: String,
    normalized_preview: Value,
    record_count: usize,
    used_api_key: bool,
    elapsed_ms: u128,
    tested_at: String,
}

struct FetchPreview {
    provider: String,
    url: String,
    normalized: Value,
    used_api_key: bool,
}

type ValidatedSource = (String, String, i64, Option<String>, String, bool);
type ValidatedJob = (String, String, String, i64, String, String, bool);

fn default_priority() -> i64 {
    100
}
fn default_timezone() -> String {
    "UTC".to_owned()
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/data-sources", get(list_sources).post(create_source))
        .route(
            "/data-sources/{id}",
            get(get_source).put(update_source).delete(delete_source),
        )
        .route("/sync-jobs", get(list_jobs).post(create_job))
        .route(
            "/sync-jobs/{id}",
            get(get_job).put(update_job).delete(delete_job),
        )
        .route("/sync-jobs/{id}/run", post(run_job))
        .route("/sync-runs", get(list_runs))
        .route(
            "/api-collectors",
            get(list_collectors).post(create_collector),
        )
        .route("/api-collectors/test", post(test_collector))
        .route(
            "/api-collectors/{id}",
            get(get_collector)
                .put(update_collector)
                .delete(delete_collector),
        )
        .route("/api-collectors/{id}/run", post(run_collector))
}

const COLLECTOR_SELECT: &str = r#"
    SELECT j.id,
           s.id AS source_id,
           s.name,
           s.source_type,
           s.priority,
           s.config_json,
           j.data_type,
           j.interval_seconds,
           j.timezone,
           j.next_run_at,
           j.last_run_at,
           CASE WHEN s.is_enabled = 1 AND j.is_enabled = 1 THEN 1 ELSE 0 END AS is_enabled,
           j.created_at,
           j.updated_at,
           (SELECT r.status FROM sync_runs r WHERE r.job_id = j.id ORDER BY r.started_at DESC LIMIT 1) AS latest_run_status,
           (SELECT r.started_at FROM sync_runs r WHERE r.job_id = j.id ORDER BY r.started_at DESC LIMIT 1) AS latest_run_at,
           (SELECT r.error_message FROM sync_runs r WHERE r.job_id = j.id ORDER BY r.started_at DESC LIMIT 1) AS latest_error,
           EXISTS(SELECT 1 FROM api_credentials c WHERE c.data_source_id = s.id) AS has_api_key
      FROM sync_jobs j
      JOIN data_sources s ON s.id = j.data_source_id
     WHERE j.deleted_at IS NULL AND s.deleted_at IS NULL
"#;

async fn list_collectors(State(state): State<AppState>) -> AppResult<Json<Vec<ApiCollectorView>>> {
    let sql = format!("{COLLECTOR_SELECT} ORDER BY j.is_enabled DESC, s.priority, s.name");
    let rows = sqlx::query_as::<_, ApiCollectorRow>(&sql)
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(collector_view).collect()))
}

async fn get_collector(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<ApiCollectorView>> {
    Ok(Json(collector_view(
        fetch_collector(&state, &common::id(&id, "采集器 ID")?).await?,
    )))
}

async fn create_collector(
    State(state): State<AppState>,
    Json(input): Json<ApiCollectorInput>,
) -> AppResult<(StatusCode, Json<ApiCollectorView>)> {
    let source_id = Uuid::now_v7().to_string();
    let job_id = Uuid::now_v7().to_string();
    let api_key = normalize_api_key(input.api_key.as_deref())?;
    let (mut source, job) = validate_collector(input, &source_id)?;
    source.3 = api_key.as_ref().map(|_| "local:encrypted-vault".to_owned());
    let next = (Utc::now() + Duration::seconds(job.3)).to_rfc3339_opts(SecondsFormat::Millis, true);
    let mut tx = state.db.begin().await?;
    sqlx::query("INSERT INTO data_sources(id,name,source_type,priority,credentials_ref,config_json,is_enabled) VALUES(?,?,?,?,?,?,?)")
        .bind(&source_id).bind(source.0).bind(source.1).bind(source.2).bind(source.3).bind(source.4).bind(source.5).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO sync_jobs(id,data_source_id,name,data_type,interval_seconds,timezone,retry_policy_json,next_run_at,is_enabled) VALUES(?,?,?,?,?,?,?,?,?)")
        .bind(&job_id).bind(job.0).bind(job.1).bind(job.2).bind(job.3).bind(job.4).bind(job.5).bind(next).bind(job.6).execute(&mut *tx).await?;
    if let Some(api_key) = api_key {
        sqlx::query("INSERT INTO api_credentials(data_source_id,encrypted_secret) VALUES(?,?)")
            .bind(&source_id)
            .bind(encrypt_secret(&api_key)?)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(collector_view(fetch_collector(&state, &job_id).await?)),
    ))
}

async fn update_collector(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<ApiCollectorInput>,
) -> AppResult<Json<ApiCollectorView>> {
    let id = common::id(&id, "采集器 ID")?;
    let current = fetch_collector(&state, &id).await?;
    let api_key = normalize_api_key(input.api_key.as_deref())?;
    let clear_api_key = input.clear_api_key;
    let (mut source, job) = validate_collector(input, &current.source_id)?;
    source.3 = if clear_api_key {
        None
    } else if api_key.is_some() || current.has_api_key {
        Some("local:encrypted-vault".to_owned())
    } else {
        None
    };
    let next = (Utc::now() + Duration::seconds(job.3)).to_rfc3339_opts(SecondsFormat::Millis, true);
    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE data_sources SET name=?,source_type=?,priority=?,credentials_ref=?,config_json=?,is_enabled=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=? AND deleted_at IS NULL")
        .bind(source.0).bind(source.1).bind(source.2).bind(source.3).bind(source.4).bind(source.5).bind(&current.source_id).execute(&mut *tx).await?;
    sqlx::query("UPDATE sync_jobs SET name=?,data_type=?,interval_seconds=?,timezone=?,retry_policy_json=?,next_run_at=?,is_enabled=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=? AND deleted_at IS NULL")
        .bind(job.1).bind(job.2).bind(job.3).bind(job.4).bind(job.5).bind(next).bind(job.6).bind(&id).execute(&mut *tx).await?;
    if clear_api_key {
        sqlx::query("DELETE FROM api_credentials WHERE data_source_id=?")
            .bind(&current.source_id)
            .execute(&mut *tx)
            .await?;
    } else if let Some(api_key) = api_key {
        sqlx::query(
            r#"INSERT INTO api_credentials(data_source_id,encrypted_secret) VALUES(?,?)
               ON CONFLICT(data_source_id) DO UPDATE SET encrypted_secret=excluded.encrypted_secret,
                 updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')"#,
        )
        .bind(&current.source_id)
        .bind(encrypt_secret(&api_key)?)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(Json(collector_view(fetch_collector(&state, &id).await?)))
}

async fn delete_collector(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let id = common::id(&id, "采集器 ID")?;
    let current = fetch_collector(&state, &id).await?;
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE sync_jobs SET is_enabled=0,deleted_at=?,updated_at=? WHERE id=?")
        .bind(&now)
        .bind(&now)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sync_jobs WHERE data_source_id=? AND deleted_at IS NULL",
    )
    .bind(&current.source_id)
    .fetch_one(&mut *tx)
    .await?;
    if remaining == 0 {
        sqlx::query("DELETE FROM api_credentials WHERE data_source_id=?")
            .bind(&current.source_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE data_sources SET is_enabled=0,deleted_at=?,updated_at=? WHERE id=?")
            .bind(&now)
            .bind(&now)
            .bind(&current.source_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn run_collector(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<SyncRun>> {
    let id = common::id(&id, "采集器 ID")?;
    fetch_collector(&state, &id).await?;
    Ok(Json(execute_job(&state, &id).await?))
}

async fn test_collector(
    State(state): State<AppState>,
    Json(input): Json<ApiCollectorTestInput>,
) -> AppResult<Json<ApiCollectorTestResult>> {
    validate_data_type(&input.data_type)?;
    let api_key = normalize_api_key(input.api_key.as_deref())?;
    let source_name = common::required_text(
        input.name.as_deref().unwrap_or("连接测试"),
        "采集器名称",
        120,
    )?;
    let source_id = if let Some(id) = input.collector_id.as_deref() {
        Some(
            fetch_collector(&state, &common::id(id, "采集器 ID")?)
                .await?
                .source_id,
        )
    } else {
        None
    };
    let started = Instant::now();
    let preview = fetch_normalized(
        &state,
        &input.data_type,
        &source_name,
        &input.config,
        source_id.as_deref(),
        api_key.as_deref(),
    )
    .await?;
    let record_count = normalized_records(&preview.normalized)?.len();
    Ok(Json(ApiCollectorTestResult {
        success: true,
        provider: preview.provider,
        data_type: input.data_type,
        request_url: preview.url,
        normalized_preview: preview.normalized,
        record_count,
        used_api_key: preview.used_api_key,
        elapsed_ms: started.elapsed().as_millis(),
        tested_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
    }))
}

async fn fetch_collector(state: &AppState, id: &str) -> AppResult<ApiCollectorRow> {
    let sql = format!("{COLLECTOR_SELECT} AND j.id = ?");
    sqlx::query_as::<_, ApiCollectorRow>(&sql)
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("API 采集器不存在".to_owned()))
}

fn collector_view(row: ApiCollectorRow) -> ApiCollectorView {
    ApiCollectorView {
        id: row.id,
        source_id: row.source_id,
        name: row.name,
        source_type: row.source_type,
        priority: row.priority,
        config: public_config(&row.config_json),
        data_type: row.data_type,
        interval_seconds: row.interval_seconds,
        timezone: row.timezone,
        next_run_at: row.next_run_at,
        last_run_at: row.last_run_at,
        is_enabled: row.is_enabled,
        created_at: row.created_at,
        updated_at: row.updated_at,
        latest_run_status: row.latest_run_status,
        latest_run_at: row.latest_run_at,
        latest_error: row.latest_error,
        has_api_key: row.has_api_key,
    }
}

fn validate_collector(
    input: ApiCollectorInput,
    source_id: &str,
) -> AppResult<(ValidatedSource, ValidatedJob)> {
    let mut config = input.config;
    if let Some(object) = config.as_object_mut() {
        object.remove("api_key");
        object.remove("api_secret");
        object.remove("token");
    }
    validate_collector_config(&input.data_type, &config)?;
    let source = validate_source(DataSourceInput {
        name: input.name.clone(),
        source_type: input.source_type,
        priority: input.priority,
        credentials_ref: None,
        config,
        is_enabled: input.is_enabled,
    })?;
    let job = validate_job(SyncJobInput {
        data_source_id: source_id.to_owned(),
        name: input.name,
        data_type: input.data_type,
        interval_seconds: input.interval_seconds,
        timezone: input.timezone,
        retry_policy: json!({"max_retries":3,"backoff":"exponential"}),
        is_enabled: input.is_enabled,
    })?;
    Ok((source, job))
}

pub fn spawn_scheduler(pool: SqlitePool) {
    tokio::spawn(async move {
        let state = AppState {
            db: pool,
            auth_required: true,
        };
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            if let Err(error) = ensure_stock_quote_collectors(&state).await {
                tracing::warn!(?error, "automatic stock quote discovery failed");
            }
            if let Err(error) = run_due_jobs(&state).await {
                tracing::warn!(?error, "scheduled data sync check failed");
            }
        }
    });
}

async fn ensure_stock_quote_collectors(state: &AppState) -> AppResult<()> {
    let configs = sqlx::query_scalar::<_, String>("SELECT config_json FROM data_sources")
        .fetch_all(&state.db)
        .await?;
    let covered: HashSet<String> = configs
        .iter()
        .filter_map(|config| serde_json::from_str::<Value>(config).ok())
        .filter_map(|config| config["instrument_id"].as_str().map(str::to_owned))
        .collect();
    let instruments = sqlx::query_as::<_, StockInstrumentRow>(
        r#"SELECT id,symbol,currency FROM instruments
           WHERE is_active=1 AND asset_type IN ('stock','etf') AND currency IN ('USD','HKD')
             AND upper(ifnull(exchange,'')) <> 'SIM'"#,
    )
    .fetch_all(&state.db)
    .await?;

    for instrument in instruments {
        if covered.contains(&instrument.id) {
            continue;
        }
        let Some(quote_symbol) =
            automatic_stock_quote_symbol(&instrument.symbol, &instrument.currency)
        else {
            tracing::warn!(instrument_id=%instrument.id,symbol=%instrument.symbol,"stock symbol cannot be mapped to quote provider");
            continue;
        };
        let source_id = Uuid::now_v7().to_string();
        let job_id = Uuid::now_v7().to_string();
        let market_name = if instrument.currency == "HKD" {
            "港股"
        } else {
            "美股"
        };
        let name = format!("{} {market_name}自动行情", instrument.symbol);
        let config = json!({
            "provider":"tencent_quote",
            "instrument_id":instrument.id,
            "quote_symbol":quote_symbol,
            "currency":instrument.currency,
            "market":if market_name == "港股" { "hk" } else { "us" },
            "auto_discovered":true
        });
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let mut tx = state.db.begin().await?;
        sqlx::query("INSERT INTO data_sources(id,name,source_type,priority,config_json,is_enabled) VALUES(?,?,'market_data',100,?,1)")
            .bind(&source_id)
            .bind(&name)
            .bind(config.to_string())
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO sync_jobs(id,data_source_id,name,data_type,interval_seconds,timezone,retry_policy_json,next_run_at,is_enabled) VALUES(?,?,?,'prices',300,'Asia/Shanghai',?, ?,1)")
            .bind(&job_id)
            .bind(&source_id)
            .bind(&name)
            .bind(json!({"max_retries":3,"backoff":"exponential"}).to_string())
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
    }
    Ok(())
}

async fn list_sources(State(state): State<AppState>) -> AppResult<Json<Vec<DataSourceView>>> {
    let rows =
        sqlx::query_as::<_, DataSourceRow>("SELECT id,name,source_type,priority,credentials_ref,config_json,is_enabled,created_at,updated_at FROM data_sources WHERE deleted_at IS NULL ORDER BY priority,name")
            .fetch_all(&state.db)
            .await?;
    Ok(Json(rows.into_iter().map(source_view).collect()))
}
async fn get_source(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<DataSourceView>> {
    Ok(Json(source_view(
        fetch_source(&state, &common::id(&id, "数据源 ID")?).await?,
    )))
}
async fn create_source(
    State(state): State<AppState>,
    Json(input): Json<DataSourceInput>,
) -> AppResult<(StatusCode, Json<DataSourceView>)> {
    let id = Uuid::now_v7().to_string();
    let v = validate_source(input)?;
    sqlx::query("INSERT INTO data_sources(id,name,source_type,priority,credentials_ref,config_json,is_enabled) VALUES(?,?,?,?,?,?,?)").bind(&id).bind(v.0).bind(v.1).bind(v.2).bind(v.3).bind(v.4).bind(v.5).execute(&state.db).await?;
    Ok((
        StatusCode::CREATED,
        Json(source_view(fetch_source(&state, &id).await?)),
    ))
}
async fn update_source(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<DataSourceInput>,
) -> AppResult<Json<DataSourceView>> {
    let id = common::id(&id, "数据源 ID")?;
    let v = validate_source(input)?;
    let r=sqlx::query("UPDATE data_sources SET name=?,source_type=?,priority=?,credentials_ref=?,config_json=?,is_enabled=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=? AND deleted_at IS NULL").bind(v.0).bind(v.1).bind(v.2).bind(v.3).bind(v.4).bind(v.5).bind(&id).execute(&state.db).await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("数据源不存在".to_owned()));
    }
    Ok(Json(source_view(fetch_source(&state, &id).await?)))
}
async fn delete_source(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let id = common::id(&id, "数据源 ID")?;
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE sync_jobs SET is_enabled=0,deleted_at=?,updated_at=? WHERE data_source_id=? AND deleted_at IS NULL")
        .bind(&now).bind(&now).bind(&id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM api_credentials WHERE data_source_id=?")
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    let r = sqlx::query("UPDATE data_sources SET is_enabled=0,deleted_at=?,updated_at=? WHERE id=? AND deleted_at IS NULL")
        .bind(&now).bind(&now).bind(&id).execute(&mut *tx).await?;
    tx.commit().await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("数据源不存在".to_owned()));
    }
    Ok(StatusCode::NO_CONTENT)
}
async fn fetch_source(state: &AppState, id: &str) -> AppResult<DataSourceRow> {
    sqlx::query_as::<_, DataSourceRow>("SELECT id,name,source_type,priority,credentials_ref,config_json,is_enabled,created_at,updated_at FROM data_sources WHERE id=? AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("数据源不存在".to_owned()))
}
fn source_view(row: DataSourceRow) -> DataSourceView {
    DataSourceView {
        id: row.id,
        name: row.name,
        source_type: row.source_type,
        priority: row.priority,
        credentials_ref: row.credentials_ref,
        config: public_config(&row.config_json),
        is_enabled: row.is_enabled,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn public_config(raw: &str) -> Value {
    let mut config = serde_json::from_str::<Value>(raw).unwrap_or(Value::Null);
    if let Some(object) = config.as_object_mut() {
        object.remove("api_key");
        object.remove("api_secret");
        object.remove("token");
    }
    config
}
fn validate_source(
    input: DataSourceInput,
) -> AppResult<(String, String, i64, Option<String>, String, bool)> {
    if !matches!(
        input.source_type.as_str(),
        "market_data" | "fx" | "benchmark" | "broker" | "crypto_exchange" | "blockchain"
    ) {
        return Err(AppError::Validation("数据源类型无效".to_owned()));
    }
    if !(0..=10000).contains(&input.priority) {
        return Err(AppError::Validation(
            "优先级必须在 0 到 10000 之间".to_owned(),
        ));
    }
    Ok((
        common::required_text(&input.name, "数据源名称", 120)?,
        input.source_type,
        input.priority,
        common::optional_text(input.credentials_ref.as_deref(), "凭据引用", 200)?,
        input.config.to_string(),
        input.is_enabled,
    ))
}

async fn list_jobs(State(state): State<AppState>) -> AppResult<Json<Vec<SyncJob>>> {
    Ok(Json(
        sqlx::query_as::<_, SyncJob>("SELECT id,data_source_id,name,data_type,interval_seconds,timezone,cursor,retry_policy_json,next_run_at,last_run_at,is_enabled,created_at,updated_at FROM sync_jobs WHERE deleted_at IS NULL ORDER BY is_enabled DESC,name")
            .fetch_all(&state.db)
            .await?,
    ))
}
async fn get_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<SyncJob>> {
    Ok(Json(
        fetch_job(&state, &common::id(&id, "同步任务 ID")?).await?,
    ))
}
async fn create_job(
    State(state): State<AppState>,
    Json(input): Json<SyncJobInput>,
) -> AppResult<(StatusCode, Json<SyncJob>)> {
    let id = Uuid::now_v7().to_string();
    let v = validate_job(input)?;
    let next = (Utc::now() + Duration::seconds(v.3)).to_rfc3339_opts(SecondsFormat::Millis, true);
    sqlx::query("INSERT INTO sync_jobs(id,data_source_id,name,data_type,interval_seconds,timezone,retry_policy_json,next_run_at,is_enabled) VALUES(?,?,?,?,?,?,?,?,?)").bind(&id).bind(v.0).bind(v.1).bind(v.2).bind(v.3).bind(v.4).bind(v.5).bind(next).bind(v.6).execute(&state.db).await?;
    Ok((StatusCode::CREATED, Json(fetch_job(&state, &id).await?)))
}
async fn update_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<SyncJobInput>,
) -> AppResult<Json<SyncJob>> {
    let id = common::id(&id, "同步任务 ID")?;
    let v = validate_job(input)?;
    let next = (Utc::now() + Duration::seconds(v.3)).to_rfc3339_opts(SecondsFormat::Millis, true);
    let r=sqlx::query("UPDATE sync_jobs SET data_source_id=?,name=?,data_type=?,interval_seconds=?,timezone=?,retry_policy_json=?,next_run_at=?,is_enabled=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=? AND deleted_at IS NULL").bind(v.0).bind(v.1).bind(v.2).bind(v.3).bind(v.4).bind(v.5).bind(next).bind(v.6).bind(&id).execute(&state.db).await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("同步任务不存在".to_owned()));
    }
    Ok(Json(fetch_job(&state, &id).await?))
}
async fn delete_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let r = sqlx::query("UPDATE sync_jobs SET is_enabled=0,deleted_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'),updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=? AND deleted_at IS NULL")
        .bind(common::id(&id, "同步任务 ID")?)
        .execute(&state.db)
        .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("同步任务不存在".to_owned()));
    }
    Ok(StatusCode::NO_CONTENT)
}
async fn fetch_job(state: &AppState, id: &str) -> AppResult<SyncJob> {
    sqlx::query_as::<_, SyncJob>("SELECT id,data_source_id,name,data_type,interval_seconds,timezone,cursor,retry_policy_json,next_run_at,last_run_at,is_enabled,created_at,updated_at FROM sync_jobs WHERE id=? AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("同步任务不存在".to_owned()))
}
fn validate_job(
    input: SyncJobInput,
) -> AppResult<(String, String, String, i64, String, String, bool)> {
    if !matches!(
        input.data_type.as_str(),
        "prices" | "fx_rates" | "balances" | "transactions"
    ) {
        return Err(AppError::Validation("同步数据类型无效".to_owned()));
    }
    if input.interval_seconds < 60 || input.interval_seconds > 31_536_000 {
        return Err(AppError::Validation(
            "同步间隔必须在 60 秒到 1 年之间".to_owned(),
        ));
    }
    Ok((
        common::id(&input.data_source_id, "数据源 ID")?,
        common::required_text(&input.name, "任务名称", 120)?,
        input.data_type,
        input.interval_seconds,
        common::required_text(&input.timezone, "时区", 80)?,
        input.retry_policy.to_string(),
        input.is_enabled,
    ))
}

async fn run_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<SyncRun>> {
    Ok(Json(
        execute_job(&state, &common::id(&id, "同步任务 ID")?).await?,
    ))
}
async fn list_runs(State(state): State<AppState>) -> AppResult<Json<Vec<SyncRun>>> {
    Ok(Json(
        sqlx::query_as::<_, SyncRun>("SELECT * FROM sync_runs ORDER BY started_at DESC LIMIT 100")
            .fetch_all(&state.db)
            .await?,
    ))
}

async fn run_due_jobs(state: &AppState) -> AppResult<()> {
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let ids=sqlx::query_scalar::<_,String>("SELECT j.id FROM sync_jobs j JOIN data_sources s ON s.id=j.data_source_id WHERE j.deleted_at IS NULL AND s.deleted_at IS NULL AND j.is_enabled=1 AND s.is_enabled=1 AND (j.next_run_at IS NULL OR j.next_run_at<=?)").bind(now).fetch_all(&state.db).await?;
    for id in ids {
        if let Err(error) = execute_job(state, &id).await {
            tracing::warn!(job_id=%id,?error,"scheduled data sync failed");
        }
    }
    Ok(())
}

async fn execute_job(state: &AppState, id: &str) -> AppResult<SyncRun> {
    let job=sqlx::query_as::<_,JobSource>(r#"SELECT j.id job_id,j.data_source_id,j.data_type,j.interval_seconds,s.name source_name,s.config_json FROM sync_jobs j JOIN data_sources s ON s.id=j.data_source_id WHERE j.id=? AND j.deleted_at IS NULL AND s.deleted_at IS NULL"#).bind(id).fetch_optional(&state.db).await?.ok_or_else(||AppError::NotFound("同步任务不存在".to_owned()))?;
    let run_id = Uuid::now_v7().to_string();
    let started = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    sqlx::query("INSERT INTO sync_runs(id,job_id,started_at,status) VALUES(?,?,?,'running')")
        .bind(&run_id)
        .bind(&job.job_id)
        .bind(&started)
        .execute(&state.db)
        .await?;
    let result = fetch_and_store(state, &job, &run_id).await;
    let finished = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let next = (Utc::now() + Duration::seconds(job.interval_seconds))
        .to_rfc3339_opts(SecondsFormat::Millis, true);
    match result {
        Ok(stats) => {
            sqlx::query(
                "UPDATE sync_runs SET finished_at=?,status='succeeded',stats_json=? WHERE id=?",
            )
            .bind(&finished)
            .bind(stats.to_string())
            .bind(&run_id)
            .execute(&state.db)
            .await?;
            sqlx::query("UPDATE sync_jobs SET last_run_at=?,next_run_at=? WHERE id=?")
                .bind(&finished)
                .bind(next)
                .bind(id)
                .execute(&state.db)
                .await?;
        }
        Err(error) => {
            let message = error.to_string();
            sqlx::query(
                "UPDATE sync_runs SET finished_at=?,status='failed',error_message=? WHERE id=?",
            )
            .bind(&finished)
            .bind(&message)
            .bind(&run_id)
            .execute(&state.db)
            .await?;
            sqlx::query("UPDATE sync_jobs SET last_run_at=?,next_run_at=? WHERE id=?")
                .bind(&finished)
                .bind(next)
                .bind(id)
                .execute(&state.db)
                .await?;
        }
    }
    Ok(
        sqlx::query_as::<_, SyncRun>("SELECT * FROM sync_runs WHERE id=?")
            .bind(run_id)
            .fetch_one(&state.db)
            .await?,
    )
}

async fn fetch_and_store(state: &AppState, job: &JobSource, run_id: &str) -> AppResult<Value> {
    let config: Value = serde_json::from_str(&job.config_json)
        .map_err(|_| AppError::Validation("数据源配置不是有效 JSON".to_owned()))?;
    let preview = fetch_normalized(
        state,
        &job.data_type,
        &job.source_name,
        &config,
        Some(&job.data_source_id),
        None,
    )
    .await?;
    let provider = preview.provider;
    let url = preview.url;
    let normalized = preview.normalized;
    let records = normalized_records(&normalized)?;
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let hash = format!("{:x}", Sha256::digest(normalized.to_string().as_bytes()));
    let mut transaction = state.db.begin().await?;
    sqlx::query("INSERT OR IGNORE INTO staged_records(id,sync_run_id,payload_hash,normalized_data_json,status) VALUES(?,?,?,?,'accepted')")
        .bind(Uuid::now_v7().to_string())
        .bind(run_id)
        .bind(hash)
        .bind(normalized.to_string())
        .execute(&mut *transaction)
        .await?;
    if job.data_type == "prices" {
        for record in &records {
            sqlx::query("INSERT INTO prices(instrument_id,price_at,price,currency,source,is_manual_override) VALUES(?,?,?,?,?,0) ON CONFLICT(instrument_id,price_at,source) DO UPDATE SET price=excluded.price,currency=excluded.currency")
                .bind(normalized_text(record, "instrument_id", "标的 ID")?)
                .bind(&now)
                .bind(normalized_text(record, "price", "价格")?)
                .bind(normalized_text(record, "currency", "价格币种")?)
                .bind(normalized_text(record, "source", "数据来源")?)
                .execute(&mut *transaction)
                .await?;
        }
    } else if job.data_type == "fx_rates" {
        for record in &records {
            sqlx::query("INSERT INTO fx_rates(base_currency,quote_currency,rate_at,rate,source) VALUES(?,?,?,?,?) ON CONFLICT(base_currency,quote_currency,rate_at,source) DO UPDATE SET rate=excluded.rate")
                .bind(normalized_text(record, "base_currency", "基础币种")?)
                .bind(normalized_text(record, "quote_currency", "报价币种")?)
                .bind(&now)
                .bind(normalized_text(record, "rate", "汇率")?)
                .bind(normalized_text(record, "source", "数据来源")?)
                .execute(&mut *transaction)
                .await?;
        }
    } else {
        return Err(AppError::Validation(
            "当前版本仅自动写入价格和汇率；账户流水进入待确认区".to_owned(),
        ));
    }
    transaction.commit().await?;
    Ok(
        json!({"records":records.len(),"provider":provider,"url":url,"data_source_id":job.data_source_id}),
    )
}

async fn fetch_normalized(
    state: &AppState,
    data_type: &str,
    source_name: &str,
    config: &Value,
    data_source_id: Option<&str>,
    api_key_override: Option<&str>,
) -> AppResult<FetchPreview> {
    validate_data_type(data_type)?;
    let client =
        super::settings::http_client(&state.db, std::time::Duration::from_secs(15)).await?;
    let provider = required_config(config, "provider")?.to_owned();
    let (url, normalized, used_api_key) = match provider.as_str() {
        "tencent_quote" => {
            if data_type != "prices" {
                return Err(AppError::Validation(
                    "股票行情采集器只能写入市场价格".to_owned(),
                ));
            }
            let instrument = common::id(required_config(config, "instrument_id")?, "标的 ID")?;
            let quote_symbol =
                validate_tencent_quote_symbol(required_config(config, "quote_symbol")?)?;
            let currency = common::currency(required_config(config, "currency")?, "行情币种")?;
            let url = format!("https://qt.gtimg.cn/q={quote_symbol}");
            let body = fetch_text(&client, &url).await?;
            let price = parse_tencent_quote(&body)?;
            let record = json!({"instrument_id":instrument,"price":price,"currency":currency,"source":source_name});
            (url, record, false)
        }
        "coingecko_simple" => {
            if data_type != "prices" {
                return Err(AppError::Validation(
                    "CoinGecko 采集器只能写入市场价格".to_owned(),
                ));
            }
            let vs = common::major_currency(required_config(config, "vs_currency")?, "价格币种")?
                .to_ascii_lowercase();
            let is_batch = config["assets"].is_array();
            let assets = if is_batch {
                configured_price_assets(config, "coin_id", "CoinGecko Coin ID")?
            } else {
                vec![PriceAssetMapping {
                    instrument_id: common::id(
                        required_config(config, "instrument_id")?,
                        "标的 ID",
                    )?,
                    lookup_key: common::required_text(
                        required_config(config, "coin_id")?,
                        "CoinGecko Coin ID",
                        120,
                    )?,
                }]
            };
            let mut request_url =
                Url::parse("https://api.coingecko.com/api/v3/simple/price").expect("static URL");
            request_url
                .query_pairs_mut()
                .append_pair(
                    "ids",
                    &assets
                        .iter()
                        .map(|asset| asset.lookup_key.as_str())
                        .collect::<Vec<_>>()
                        .join(","),
                )
                .append_pair("vs_currencies", &vs);
            let url = request_url.to_string();
            let body = fetch_json(&client, &url).await?;
            let records = normalize_coingecko_response(&body, &assets, &vs, source_name)?;
            let normalized = if is_batch {
                Value::Array(records)
            } else {
                records.into_iter().next().expect("one configured asset")
            };
            (url, normalized, false)
        }
        "frankfurter" => {
            if data_type != "fx_rates" {
                return Err(AppError::Validation(
                    "Frankfurter 采集器只能写入汇率".to_owned(),
                ));
            }
            let base = common::major_currency(required_config(config, "base")?, "基础币种")?;
            let quote = common::major_currency(required_config(config, "quote")?, "报价币种")?;
            if base == quote {
                return Err(AppError::Validation(
                    "基础币种和报价币种不能相同".to_owned(),
                ));
            }
            let url = format!("https://api.frankfurter.dev/v2/rate/{base}/{quote}");
            let body = fetch_json(&client, &url).await?;
            let value = numeric_json_value(&body["rate"], "汇率响应中没有目标数值")?;
            let record = json!({"base_currency":base,"quote_currency":quote,"rate":value,"source":source_name});
            (url, record, false)
        }
        "generic_json" => {
            let mut url = required_config(config, "url")?.to_owned();
            if data_type == "fx_rates" {
                let base = common::major_currency(required_config(config, "base")?, "基础币种")?;
                let mode = fx_response_mode(config)?;
                url = url.replace("{base}", &base);
                if mode == "single" {
                    let quote =
                        common::major_currency(required_config(config, "quote")?, "报价币种")?;
                    url = url.replace("{quote}", &quote).replace("{quotes}", &quote);
                } else {
                    let quotes = configured_fx_quotes(config, &base)?;
                    url = url.replace("{quotes}", &quotes.join(","));
                    if url.contains("{quote}") {
                        return Err(AppError::Validation(
                            "批量汇率接口请使用 {quotes} 占位符，不要使用 {quote}".to_owned(),
                        ));
                    }
                }
            } else if data_type == "prices" && price_response_mode(config)? != "single" {
                let assets = configured_price_assets(config, "lookup_key", "接口识别键")?;
                url = url.replace(
                    "{symbols}",
                    &assets
                        .iter()
                        .map(|asset| asset.lookup_key.as_str())
                        .collect::<Vec<_>>()
                        .join(","),
                );
                if url.contains("{symbol}") {
                    return Err(AppError::Validation(
                        "批量价格接口请使用 {symbols} 占位符，不要使用 {symbol}".to_owned(),
                    ));
                }
            }
            validate_public_url(&url)?;
            let auth_type = config["auth_type"].as_str().unwrap_or("none");
            let api_key = if auth_type == "none" {
                None
            } else {
                Some(
                    resolve_api_key(&state.db, data_source_id, api_key_override)
                        .await?
                        .ok_or_else(|| {
                            AppError::Validation("该接口需要 API Key，请先填写密钥".to_owned())
                        })?,
                )
            };
            let key_name = config["api_key_name"].as_str();
            let body = fetch_json_with_auth(&client, &url, auth_type, key_name, api_key.as_deref())
                .await?;
            let record = if data_type == "prices" {
                normalize_generic_price_response(&body, config, source_name)?
            } else if data_type == "fx_rates" {
                normalize_generic_fx_response(&body, config, source_name)?
            } else {
                return Err(AppError::Validation(
                    "通用 JSON API 当前只支持价格和汇率".to_owned(),
                ));
            };
            (url, record, api_key.is_some())
        }
        _ => return Err(AppError::Validation("暂不支持该 API provider".to_owned())),
    };
    Ok(FetchPreview {
        provider,
        url,
        normalized,
        used_api_key,
    })
}

async fn fetch_json(client: &reqwest::Client, url: &str) -> AppResult<Value> {
    validate_resolved_public_url(url).await?;
    let response = client
        .get(url)
        .header("user-agent", "SANYU-Invest/0.1")
        .send()
        .await
        .map_err(|e| AppError::External(format!("API 请求失败：{e}")))?;
    if !response.status().is_success() {
        return Err(AppError::External(format!(
            "API 返回状态 {}",
            response.status()
        )));
    }
    response
        .json::<Value>()
        .await
        .map_err(|e| AppError::External(format!("API 返回的不是有效 JSON：{e}")))
}

async fn fetch_json_with_auth(
    client: &reqwest::Client,
    url: &str,
    auth_type: &str,
    api_key_name: Option<&str>,
    api_key: Option<&str>,
) -> AppResult<Value> {
    validate_resolved_public_url(url).await?;
    let mut parsed =
        Url::parse(url).map_err(|_| AppError::Validation("API 地址无效".to_owned()))?;
    let mut request = match auth_type {
        "none" => client.get(parsed.clone()),
        "query" => {
            let name =
                common::required_text(api_key_name.unwrap_or("apikey"), "API Key 查询参数名", 80)?;
            let key = api_key.ok_or_else(|| AppError::Validation("缺少 API Key".to_owned()))?;
            parsed.query_pairs_mut().append_pair(&name, key);
            client.get(parsed)
        }
        "header" => {
            let name = common::required_text(
                api_key_name.unwrap_or("X-API-Key"),
                "API Key 请求头名称",
                80,
            )?;
            let header_name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| AppError::Validation("API Key 请求头名称格式无效".to_owned()))?;
            let header_value = HeaderValue::from_str(
                api_key.ok_or_else(|| AppError::Validation("缺少 API Key".to_owned()))?,
            )
            .map_err(|_| AppError::Validation("API Key 包含无效字符".to_owned()))?;
            client.get(parsed).header(header_name, header_value)
        }
        "bearer" => client
            .get(parsed)
            .bearer_auth(api_key.ok_or_else(|| AppError::Validation("缺少 API Key".to_owned()))?),
        _ => {
            return Err(AppError::Validation(
                "认证方式必须是 none、header、query 或 bearer".to_owned(),
            ));
        }
    };
    request = request.header("user-agent", "SANYU-Invest/0.1");
    let response = request.send().await.map_err(|error| {
        if auth_type == "none" {
            AppError::External(format!("API 请求失败：{error}"))
        } else {
            AppError::External("API 请求失败，包含凭据的底层信息已隐藏".to_owned())
        }
    })?;
    if !response.status().is_success() {
        return Err(AppError::External(format!(
            "API 返回状态 {}",
            response.status()
        )));
    }
    response
        .json::<Value>()
        .await
        .map_err(|error| AppError::External(format!("API 返回的不是有效 JSON：{error}")))
}

async fn fetch_text(client: &reqwest::Client, url: &str) -> AppResult<String> {
    validate_resolved_public_url(url).await?;
    let response = client
        .get(url)
        .header("user-agent", "SANYU-Invest/0.1")
        .send()
        .await
        .map_err(|e| AppError::External(format!("API 请求失败：{e}")))?;
    if !response.status().is_success() {
        return Err(AppError::External(format!(
            "API 返回状态 {}",
            response.status()
        )));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| AppError::External(format!("API 响应读取失败：{e}")))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn parse_tencent_quote(body: &str) -> AppResult<String> {
    let line = body
        .lines()
        .find(|line| line.contains("=\""))
        .ok_or_else(|| AppError::External("股票行情响应为空".to_owned()))?;
    let (_, payload) = line
        .split_once("=\"")
        .ok_or_else(|| AppError::External("股票行情响应格式无效".to_owned()))?;
    let payload = payload.trim().trim_end_matches(';').trim_end_matches('"');
    let fields: Vec<&str> = payload.split('~').collect();
    if fields.first().copied() == Some("v_pv_none_match") || fields.len() < 4 {
        return Err(AppError::External("未找到对应的股票行情代码".to_owned()));
    }
    common::positive_decimal(fields[3], "股票行情价格")
        .map_err(|_| AppError::External("股票行情响应中没有有效价格".to_owned()))
}

fn validate_tencent_quote_symbol(value: &str) -> AppResult<String> {
    let value = value.trim();
    let valid_prefix = value.starts_with("us") || value.starts_with("hk");
    let valid_suffix = value.get(2..).is_some_and(|suffix| {
        !suffix.is_empty()
            && suffix.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
            })
    });
    if !valid_prefix || !valid_suffix {
        return Err(AppError::Validation(
            "行情代码必须以 us 或 hk 开头，例如 usAAPL、hk00700".to_owned(),
        ));
    }
    Ok(value.to_owned())
}

fn validate_data_type(value: &str) -> AppResult<()> {
    if matches!(value, "prices" | "fx_rates") {
        Ok(())
    } else {
        Err(AppError::Validation(
            "连接测试目前支持市场价格和汇率".to_owned(),
        ))
    }
}

#[derive(Debug, Clone)]
struct PriceAssetMapping {
    instrument_id: String,
    lookup_key: String,
}

fn price_response_mode(config: &Value) -> AppResult<&str> {
    let mode = config["response_mode"].as_str().unwrap_or("single");
    if matches!(mode, "single" | "asset_map" | "asset_list") {
        Ok(mode)
    } else {
        Err(AppError::Validation(
            "价格响应模式必须是 single、asset_map 或 asset_list".to_owned(),
        ))
    }
}

fn configured_price_assets(
    config: &Value,
    key_name: &str,
    key_label: &str,
) -> AppResult<Vec<PriceAssetMapping>> {
    let values = config["assets"]
        .as_array()
        .ok_or_else(|| AppError::Validation("批量价格采集器至少要选择一个标的".to_owned()))?;
    let mut assets = Vec::new();
    let mut instrument_ids = HashSet::new();
    let mut lookup_keys = HashSet::new();
    for value in values {
        let instrument_id = common::id(
            value["instrument_id"]
                .as_str()
                .ok_or_else(|| AppError::Validation("批量价格标的缺少 instrument_id".to_owned()))?,
            "标的 ID",
        )?;
        let lookup_key = common::required_text(
            value[key_name]
                .as_str()
                .ok_or_else(|| AppError::Validation(format!("批量价格标的缺少 {key_name}")))?,
            key_label,
            120,
        )?;
        let normalized_key = lookup_key.to_ascii_lowercase();
        if !instrument_ids.insert(instrument_id.clone()) {
            return Err(AppError::Validation("批量价格标的不能重复".to_owned()));
        }
        if !lookup_keys.insert(normalized_key) {
            return Err(AppError::Validation(format!("{key_label} 不能重复")));
        }
        assets.push(PriceAssetMapping {
            instrument_id,
            lookup_key,
        });
    }
    if assets.is_empty() {
        return Err(AppError::Validation(
            "批量价格采集器至少要选择一个标的".to_owned(),
        ));
    }
    Ok(assets)
}

fn normalize_coingecko_response(
    body: &Value,
    assets: &[PriceAssetMapping],
    vs_currency: &str,
    source_name: &str,
) -> AppResult<Vec<Value>> {
    let mut records = Vec::with_capacity(assets.len());
    for asset in assets {
        let value = body
            .as_object()
            .and_then(|items| {
                items
                    .iter()
                    .find(|(key, _)| key.eq_ignore_ascii_case(&asset.lookup_key))
                    .map(|(_, value)| value)
            })
            .and_then(|value| {
                value.as_object().and_then(|prices| {
                    prices
                        .iter()
                        .find(|(key, _)| key.eq_ignore_ascii_case(vs_currency))
                        .map(|(_, value)| value)
                })
            })
            .unwrap_or(&Value::Null);
        let price = numeric_json_value(
            value,
            &format!("CoinGecko 响应中缺少 {} 价格", asset.lookup_key),
        )?;
        records.push(json!({
            "instrument_id": asset.instrument_id,
            "price": price,
            "currency": vs_currency.to_ascii_uppercase(),
            "source": source_name
        }));
    }
    Ok(records)
}

fn normalize_generic_price_response(
    body: &Value,
    config: &Value,
    source_name: &str,
) -> AppResult<Value> {
    let mode = price_response_mode(config)?;
    let currency = common::currency(required_config(config, "currency")?, "价格币种")?;
    if mode == "single" {
        let path = required_config(config, "value_path")?;
        let value = numeric_json_value(
            json_path(body, path).unwrap_or(&Value::Null),
            "通用 API 响应中没有目标数值，请检查字段路径",
        )?;
        return Ok(json!({
            "instrument_id": common::id(required_config(config,"instrument_id")?,"标的 ID")?,
            "price": value,
            "currency": currency,
            "source": source_name
        }));
    }

    let assets = configured_price_assets(config, "lookup_key", "接口识别键")?;
    let prices_path = required_config(config, "prices_path")?;
    let prices = json_path_or_root(body, prices_path).ok_or_else(|| {
        AppError::External("通用 API 响应中没有批量价格容器，请检查价格集合路径".to_owned())
    })?;
    let price_field = config["price_field"].as_str().unwrap_or("").trim();
    let mut records = Vec::with_capacity(assets.len());

    if mode == "asset_map" {
        let map = prices.as_object().ok_or_else(|| {
            AppError::External("价格集合路径必须指向以币种代码为键的 JSON 对象".to_owned())
        })?;
        for asset in assets {
            let value = map
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(&asset.lookup_key))
                .map(|(_, value)| value)
                .ok_or_else(|| {
                    AppError::External(format!("API 响应中缺少 {} 价格", asset.lookup_key))
                })?;
            let value = if price_field.is_empty() {
                value
            } else {
                json_path(value, price_field).unwrap_or(&Value::Null)
            };
            let price =
                numeric_json_value(value, &format!("{} 价格不是有效正数", asset.lookup_key))?;
            records.push(json!({"instrument_id":asset.instrument_id,"price":price,"currency":currency,"source":source_name}));
        }
    } else {
        let list = prices
            .as_array()
            .ok_or_else(|| AppError::External("价格集合路径必须指向 JSON 数组".to_owned()))?;
        let symbol_field = required_config(config, "symbol_field")?;
        let price_field = required_config(config, "price_field")?;
        for asset in assets {
            let item = list
                .iter()
                .find(|item| {
                    json_path(item, symbol_field)
                        .and_then(Value::as_str)
                        .is_some_and(|value| value.eq_ignore_ascii_case(&asset.lookup_key))
                })
                .ok_or_else(|| {
                    AppError::External(format!("API 响应数组中缺少 {} 价格", asset.lookup_key))
                })?;
            let price = numeric_json_value(
                json_path(item, price_field).unwrap_or(&Value::Null),
                &format!("{} 价格不是有效正数", asset.lookup_key),
            )?;
            records.push(json!({"instrument_id":asset.instrument_id,"price":price,"currency":currency,"source":source_name}));
        }
    }
    Ok(Value::Array(records))
}

fn fx_response_mode(config: &Value) -> AppResult<&str> {
    let mode = config["response_mode"].as_str().unwrap_or("single");
    if matches!(
        mode,
        "single" | "currency_paths" | "currency_map" | "currency_list"
    ) {
        Ok(mode)
    } else {
        Err(AppError::Validation(
            "汇率响应模式必须是 single、currency_paths、currency_map 或 currency_list".to_owned(),
        ))
    }
}

fn configured_fx_quotes(config: &Value, base: &str) -> AppResult<Vec<String>> {
    let values = config["quotes"]
        .as_array()
        .ok_or_else(|| AppError::Validation("批量汇率采集器至少要选择一个报价币种".to_owned()))?;
    let mut quotes = Vec::new();
    for value in values {
        let raw = value
            .as_str()
            .ok_or_else(|| AppError::Validation("批量报价币种必须是字符串数组".to_owned()))?;
        let quote = common::major_currency(raw, "报价币种")?;
        if quote == base {
            return Err(AppError::Validation(
                "批量报价币种不能包含基础币种".to_owned(),
            ));
        }
        if !quotes.contains(&quote) {
            quotes.push(quote);
        }
    }
    if quotes.is_empty() {
        return Err(AppError::Validation(
            "批量汇率采集器至少要选择一个报价币种".to_owned(),
        ));
    }
    Ok(quotes)
}

fn configured_fx_value_paths(
    config: &Value,
    quotes: &[String],
) -> AppResult<Vec<(String, String)>> {
    let values = config["value_paths"].as_object().ok_or_else(|| {
        AppError::Validation("独立字段路径模式必须为每个报价币种配置数值字段路径".to_owned())
    })?;
    let mut mappings = Vec::with_capacity(quotes.len());
    for quote in quotes {
        let path = values
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(quote))
            .and_then(|(_, value)| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AppError::Validation(format!("{quote} 缺少数值字段路径")))?;
        mappings.push((quote.clone(), path.to_owned()));
    }
    Ok(mappings)
}

fn validate_collector_config(data_type: &str, config: &Value) -> AppResult<()> {
    let provider = required_config(config, "provider")?;
    if data_type == "prices" && provider == "coingecko_simple" {
        common::major_currency(required_config(config, "vs_currency")?, "价格币种")?;
        if config["assets"].is_array() {
            configured_price_assets(config, "coin_id", "CoinGecko Coin ID")?;
        } else {
            common::id(required_config(config, "instrument_id")?, "标的 ID")?;
            common::required_text(
                required_config(config, "coin_id")?,
                "CoinGecko Coin ID",
                120,
            )?;
        }
        return Ok(());
    }

    if data_type == "prices" && provider == "generic_json" {
        required_config(config, "url")?;
        common::currency(required_config(config, "currency")?, "价格币种")?;
        let mode = price_response_mode(config)?;
        if mode == "single" {
            common::id(required_config(config, "instrument_id")?, "标的 ID")?;
            required_config(config, "value_path")?;
            return Ok(());
        }
        configured_price_assets(config, "lookup_key", "接口识别键")?;
        required_config(config, "prices_path")?;
        if mode == "asset_list" {
            required_config(config, "symbol_field")?;
            required_config(config, "price_field")?;
        }
        return Ok(());
    }

    if data_type != "fx_rates" || provider != "generic_json" {
        return Ok(());
    }

    required_config(config, "url")?;
    let base = common::major_currency(required_config(config, "base")?, "基础币种")?;
    let mode = fx_response_mode(config)?;
    if mode == "single" {
        let quote = common::major_currency(required_config(config, "quote")?, "报价币种")?;
        if base == quote {
            return Err(AppError::Validation(
                "基础币种和报价币种不能相同".to_owned(),
            ));
        }
        required_config(config, "value_path")?;
        return Ok(());
    }

    let quotes = configured_fx_quotes(config, &base)?;
    if mode == "currency_paths" {
        configured_fx_value_paths(config, &quotes)?;
        return Ok(());
    }
    required_config(config, "rates_path")?;
    if mode == "currency_list" {
        required_config(config, "currency_field")?;
        required_config(config, "rate_field")?;
    }
    Ok(())
}

fn normalize_generic_fx_response(
    body: &Value,
    config: &Value,
    source_name: &str,
) -> AppResult<Value> {
    let base = common::major_currency(required_config(config, "base")?, "基础币种")?;
    let mode = fx_response_mode(config)?;
    if mode == "single" {
        let quote = common::major_currency(required_config(config, "quote")?, "报价币种")?;
        if base == quote {
            return Err(AppError::Validation(
                "基础币种和报价币种不能相同".to_owned(),
            ));
        }
        let path = required_config(config, "value_path")?;
        let rate = numeric_json_value(
            json_path(body, path).unwrap_or(&Value::Null),
            "通用 API 响应中没有目标汇率，请检查数值字段路径",
        )?;
        return Ok(
            json!({"base_currency":base,"quote_currency":quote,"rate":rate,"source":source_name}),
        );
    }

    let quotes = configured_fx_quotes(config, &base)?;
    if mode == "currency_paths" {
        let mappings = configured_fx_value_paths(config, &quotes)?;
        let mut records = Vec::with_capacity(mappings.len());
        for (quote, path) in mappings {
            let rate = numeric_json_value(
                json_path_or_root(body, &path).unwrap_or(&Value::Null),
                &format!("{quote} 数值字段路径没有返回有效正数"),
            )?;
            records.push(json!({"base_currency":base,"quote_currency":quote,"rate":rate,"source":source_name}));
        }
        return Ok(Value::Array(records));
    }
    let rates_path = required_config(config, "rates_path")?;
    let rates = json_path_or_root(body, rates_path).ok_or_else(|| {
        AppError::External("通用 API 响应中没有批量汇率容器，请检查汇率集合路径".to_owned())
    })?;
    let mut records = Vec::with_capacity(quotes.len());

    if mode == "currency_map" {
        let map = rates.as_object().ok_or_else(|| {
            AppError::External("汇率集合路径必须指向以货币代码为键的 JSON 对象".to_owned())
        })?;
        let rate_field = config["rate_field"].as_str().unwrap_or("").trim();
        for quote in quotes {
            let value = map
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(&quote))
                .map(|(_, value)| value)
                .ok_or_else(|| AppError::External(format!("API 响应中缺少 {quote} 汇率")))?;
            let value = if rate_field.is_empty() {
                value
            } else {
                json_path(value, rate_field).unwrap_or(&Value::Null)
            };
            let rate = numeric_json_value(value, &format!("{quote} 汇率不是有效正数"))?;
            records.push(json!({"base_currency":base,"quote_currency":quote,"rate":rate,"source":source_name}));
        }
    } else {
        let list = rates
            .as_array()
            .ok_or_else(|| AppError::External("汇率集合路径必须指向 JSON 数组".to_owned()))?;
        let currency_field = config["currency_field"]
            .as_str()
            .unwrap_or("currency")
            .trim();
        let rate_field = config["rate_field"].as_str().unwrap_or("rate").trim();
        if currency_field.is_empty() || rate_field.is_empty() {
            return Err(AppError::Validation(
                "数组模式必须填写货币代码字段和汇率数值字段".to_owned(),
            ));
        }
        for quote in quotes {
            let item = list
                .iter()
                .find(|item| {
                    json_path(item, currency_field)
                        .and_then(Value::as_str)
                        .is_some_and(|value| value.eq_ignore_ascii_case(&quote))
                })
                .ok_or_else(|| AppError::External(format!("API 响应数组中缺少 {quote} 汇率")))?;
            let rate = numeric_json_value(
                json_path(item, rate_field).unwrap_or(&Value::Null),
                &format!("{quote} 汇率不是有效正数"),
            )?;
            records.push(json!({"base_currency":base,"quote_currency":quote,"rate":rate,"source":source_name}));
        }
    }

    Ok(Value::Array(records))
}

fn json_path_or_root<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    if matches!(path.trim(), "" | "$") {
        Some(value)
    } else {
        json_path(value, path)
    }
}

fn normalized_records(value: &Value) -> AppResult<Vec<&Value>> {
    match value {
        Value::Array(records) if !records.is_empty() => Ok(records.iter().collect()),
        Value::Object(_) => Ok(vec![value]),
        Value::Array(_) => Err(AppError::External(
            "API 响应没有可写入的标准化记录".to_owned(),
        )),
        _ => Err(AppError::External(
            "API 标准化结果必须是对象或对象数组".to_owned(),
        )),
    }
}

fn normalized_text<'a>(record: &'a Value, key: &str, label: &str) -> AppResult<&'a str> {
    record[key]
        .as_str()
        .ok_or_else(|| AppError::External(format!("标准化记录缺少{label}")))
}

fn numeric_json_value(value: &Value, message: &str) -> AppResult<String> {
    let raw = match value {
        Value::Number(number) => number.to_string(),
        Value::String(value) => value.trim().to_owned(),
        _ => return Err(AppError::External(message.to_owned())),
    };
    common::positive_decimal(&raw, "API 数值").map_err(|_| AppError::External(message.to_owned()))
}

fn normalize_api_key(value: Option<&str>) -> AppResult<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > 4096 {
        return Err(AppError::Validation(
            "API Key 不能超过 4096 个字符".to_owned(),
        ));
    }
    Ok(Some(value.to_owned()))
}

async fn resolve_api_key(
    pool: &SqlitePool,
    data_source_id: Option<&str>,
    override_value: Option<&str>,
) -> AppResult<Option<String>> {
    if let Some(value) = normalize_api_key(override_value)? {
        return Ok(Some(value));
    }
    let Some(data_source_id) = data_source_id else {
        return Ok(None);
    };
    let encrypted = sqlx::query_scalar::<_, String>(
        "SELECT encrypted_secret FROM api_credentials WHERE data_source_id=?",
    )
    .bind(data_source_id)
    .fetch_optional(pool)
    .await?;
    encrypted.map(|value| decrypt_secret(&value)).transpose()
}

static CREDENTIAL_KEY: OnceLock<[u8; 32]> = OnceLock::new();

fn credential_key() -> AppResult<&'static [u8; 32]> {
    if let Some(key) = CREDENTIAL_KEY.get() {
        return Ok(key);
    }
    let key = if let Ok(value) = std::env::var("SANYU_CREDENTIAL_MASTER_KEY") {
        Sha256::digest(value.as_bytes()).into()
    } else {
        load_or_create_credential_key()?
    };
    let _ = CREDENTIAL_KEY.set(key);
    CREDENTIAL_KEY
        .get()
        .ok_or_else(|| AppError::External("无法初始化本机凭据密钥".to_owned()))
}

fn load_or_create_credential_key() -> AppResult<[u8; 32]> {
    let path = std::env::var("SANYU_CREDENTIAL_KEY_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data/credentials.key"));
    if path.exists() {
        return parse_credential_key(
            &fs::read_to_string(&path)
                .map_err(|_| AppError::External("无法读取本机凭据密钥文件".to_owned()))?,
        );
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|_| AppError::External("无法创建凭据目录".to_owned()))?;
    }
    let mut key = [0_u8; 32];
    SystemRandom::new()
        .fill(&mut key)
        .map_err(|_| AppError::External("无法生成本机凭据密钥".to_owned()))?;
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => file
            .write_all(hex_encode(&key).as_bytes())
            .map_err(|_| AppError::External("无法保存本机凭据密钥".to_owned()))?,
        Err(_) if path.exists() => {
            return parse_credential_key(
                &fs::read_to_string(&path)
                    .map_err(|_| AppError::External("无法读取本机凭据密钥文件".to_owned()))?,
            );
        }
        Err(_) => return Err(AppError::External("无法保存本机凭据密钥".to_owned())),
    }
    Ok(key)
}

fn parse_credential_key(value: &str) -> AppResult<[u8; 32]> {
    let decoded = hex_decode(value.trim())?;
    decoded
        .try_into()
        .map_err(|_| AppError::External("本机凭据密钥长度无效".to_owned()))
}

fn encrypt_secret(value: &str) -> AppResult<String> {
    let key = LessSafeKey::new(
        UnboundKey::new(&AES_256_GCM, credential_key()?)
            .map_err(|_| AppError::External("无法初始化凭据加密".to_owned()))?,
    );
    let mut nonce_bytes = [0_u8; 12];
    SystemRandom::new()
        .fill(&mut nonce_bytes)
        .map_err(|_| AppError::External("无法生成凭据随机数".to_owned()))?;
    let mut buffer = value.as_bytes().to_vec();
    key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce_bytes),
        Aad::empty(),
        &mut buffer,
    )
    .map_err(|_| AppError::External("API Key 加密失败".to_owned()))?;
    Ok(format!(
        "v1:{}:{}",
        hex_encode(&nonce_bytes),
        hex_encode(&buffer)
    ))
}

fn decrypt_secret(value: &str) -> AppResult<String> {
    let mut parts = value.split(':');
    if parts.next() != Some("v1") {
        return Err(AppError::External("API Key 密文版本无效".to_owned()));
    }
    let nonce: [u8; 12] = hex_decode(parts.next().unwrap_or(""))?
        .try_into()
        .map_err(|_| AppError::External("API Key 密文随机数无效".to_owned()))?;
    let mut encrypted = hex_decode(parts.next().unwrap_or(""))?;
    if parts.next().is_some() {
        return Err(AppError::External("API Key 密文格式无效".to_owned()));
    }
    let key = LessSafeKey::new(
        UnboundKey::new(&AES_256_GCM, credential_key()?)
            .map_err(|_| AppError::External("无法初始化凭据解密".to_owned()))?,
    );
    let plain = key
        .open_in_place(
            Nonce::assume_unique_for_key(nonce),
            Aad::empty(),
            &mut encrypted,
        )
        .map_err(|_| AppError::External("API Key 解密失败".to_owned()))?;
    String::from_utf8(plain.to_vec())
        .map_err(|_| AppError::External("API Key 明文格式无效".to_owned()))
}

fn hex_encode(value: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn hex_decode(value: &str) -> AppResult<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return Err(AppError::External("本机凭据密钥格式无效".to_owned()));
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|chunk| {
            let text = std::str::from_utf8(chunk)
                .map_err(|_| AppError::External("本机凭据密钥格式无效".to_owned()))?;
            u8::from_str_radix(text, 16)
                .map_err(|_| AppError::External("本机凭据密钥格式无效".to_owned()))
        })
        .collect()
}

fn automatic_stock_quote_symbol(symbol: &str, currency: &str) -> Option<String> {
    let symbol = symbol.trim().to_ascii_uppercase();
    match currency {
        "HKD" => {
            let code = symbol.strip_prefix("HK").unwrap_or(&symbol);
            let code = code.strip_suffix(".HK").unwrap_or(code);
            if code.is_empty()
                || !code.chars().all(|character| character.is_ascii_digit())
                || code.len() > 5
            {
                return None;
            }
            Some(format!("hk{code:0>5}"))
        }
        "USD" => {
            let code = symbol.strip_prefix("US").unwrap_or(&symbol);
            let code = code.strip_suffix(".US").unwrap_or(code);
            if code.is_empty()
                || !code.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
                })
            {
                return None;
            }
            Some(format!("us{code}"))
        }
        _ => None,
    }
}
fn validate_public_url(value: &str) -> AppResult<()> {
    let url = Url::parse(value).map_err(|_| AppError::Validation("API 地址无效".to_owned()))?;
    if url.scheme() != "https" {
        return Err(AppError::Validation(
            "定时 API 仅允许 HTTPS 地址".to_owned(),
        ));
    }
    let host = url
        .host_str()
        .unwrap_or("")
        .trim_matches(['[', ']'])
        .to_ascii_lowercase();
    let blocked_name = host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal");
    let blocked_ip = host.parse::<IpAddr>().ok().is_some_and(is_non_public_ip);
    if blocked_name || blocked_ip {
        return Err(AppError::Validation(
            "API 地址不能指向本机、私有网络或保留地址".to_owned(),
        ));
    }
    Ok(())
}

async fn validate_resolved_public_url(value: &str) -> AppResult<()> {
    validate_public_url(value)?;
    let url = Url::parse(value).map_err(|_| AppError::Validation("API 地址无效".to_owned()))?;
    let host = url
        .host_str()
        .ok_or_else(|| AppError::Validation("API 地址缺少主机名".to_owned()))?
        .trim_matches(['[', ']']);
    if host.parse::<IpAddr>().is_ok() {
        return Ok(());
    }
    let port = url.port_or_known_default().unwrap_or(443);
    let addresses = tokio::net::lookup_host((host, port))
        .await
        .map_err(|error| AppError::External(format!("API 域名解析失败：{error}")))?;
    let mut found = false;
    for address in addresses {
        found = true;
        if is_non_public_ip(address.ip()) {
            return Err(AppError::Validation(
                "API 域名解析到了本机、私有网络或保留地址".to_owned(),
            ));
        }
    }
    if !found {
        return Err(AppError::External("API 域名没有可用地址".to_owned()));
    }
    Ok(())
}

fn is_non_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_unspecified()
                || ip.is_multicast()
                || a == 0
                || (a == 100 && (64..=127).contains(&b))
                || (a == 192 && b == 0 && c == 0)
                || (a == 198 && matches!(b, 18 | 19))
                || a >= 240
        }
        IpAddr::V6(ip) => {
            let segments = ip.segments();
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || (segments[0] & 0xfe00) == 0xfc00
                || (segments[0] & 0xffc0) == 0xfe80
                || (segments[0] == 0x2001 && segments[1] == 0x0db8)
                || ip
                    .to_ipv4_mapped()
                    .is_some_and(|mapped| is_non_public_ip(IpAddr::V4(mapped)))
        }
    }
}
fn required_config<'a>(config: &'a Value, key: &str) -> AppResult<&'a str> {
    config[key]
        .as_str()
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| AppError::Validation(format!("数据源配置缺少 {key}")))
}
fn json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.trim_matches('.')
        .split('.')
        .try_fold(value, |current, key| match current {
            Value::Array(items) => key.parse::<usize>().ok().and_then(|index| items.get(index)),
            _ => current.get(key),
        })
}

#[cfg(test)]
mod tests {
    use super::{
        AppState, PriceAssetMapping, automatic_stock_quote_symbol, decrypt_secret, encrypt_secret,
        ensure_stock_quote_collectors, json_path, normalize_coingecko_response,
        normalize_generic_fx_response, normalize_generic_price_response, normalized_records,
        numeric_json_value, parse_tencent_quote, validate_collector_config, validate_public_url,
    };
    use crate::db;
    use serde_json::Value;
    use uuid::Uuid;

    #[test]
    fn maps_us_and_hong_kong_symbols_for_public_quotes() {
        assert_eq!(
            automatic_stock_quote_symbol("AAPL", "USD").as_deref(),
            Some("usAAPL")
        );
        assert_eq!(
            automatic_stock_quote_symbol("700.HK", "HKD").as_deref(),
            Some("hk00700")
        );
        assert_eq!(
            automatic_stock_quote_symbol("00700", "HKD").as_deref(),
            Some("hk00700")
        );
    }

    #[test]
    fn blocks_private_reserved_and_non_https_collector_urls() {
        for url in [
            "http://example.com/data",
            "https://localhost/data",
            "https://172.16.0.1/data",
            "https://169.254.169.254/latest/meta-data",
            "https://100.64.0.1/data",
            "https://[::1]/data",
            "https://[fc00::1]/data",
            "https://[fe80::1]/data",
        ] {
            assert!(validate_public_url(url).is_err(), "{url} 应被拒绝");
        }
        assert!(validate_public_url("https://api.example.com/data").is_ok());
    }

    #[test]
    fn parses_us_and_hong_kong_quote_prices() {
        let us =
            r#"v_usAAPL="200~Apple~AAPL.OQ~314.17~317.31~313.76~12542086~~2026-07-14 11:47:20";"#;
        let hk = r#"v_hk00700="100~Tencent~00700~456.200~457.600~457.600~25540540.0~~2026/07/14 16:09:02";"#;
        assert_eq!(parse_tencent_quote(us).unwrap(), "314.17");
        assert_eq!(parse_tencent_quote(hk).unwrap(), "456.2");
    }

    #[test]
    fn encrypts_credentials_and_supports_nested_array_value_paths() {
        let encrypted = encrypt_secret("test-api-key").unwrap();
        assert!(!encrypted.contains("test-api-key"));
        assert_eq!(decrypt_secret(&encrypted).unwrap(), "test-api-key");

        let payload = serde_json::json!({"data":{"rates":[{"value":"7.2351"}]}});
        let value = json_path(&payload, "data.rates.0.value").unwrap();
        assert_eq!(numeric_json_value(value, "missing").unwrap(), "7.2351");
    }

    #[test]
    fn normalizes_multiple_fx_rates_from_currency_map() {
        let payload = serde_json::json!({
            "data": {"rates": {"CNY": 7.2351, "EUR": "0.9234", "JPY": 158.2}}
        });
        let config = serde_json::json!({
            "base": "USD",
            "response_mode": "currency_map",
            "quotes": ["CNY", "EUR", "JPY"],
            "rates_path": "data.rates"
        });
        let normalized = normalize_generic_fx_response(&payload, &config, "test").unwrap();
        let records = normalized_records(&normalized).unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0]["quote_currency"], "CNY");
        assert_eq!(records[1]["rate"], "0.9234");
        assert_eq!(records[2]["rate"], "158.2");
    }

    #[test]
    fn normalizes_multiple_fx_rates_from_currency_list() {
        let payload = serde_json::json!({
            "items": [
                {"code": "hkd", "mid": 7.8123},
                {"code": "SGD", "mid": "1.3567"}
            ]
        });
        let config = serde_json::json!({
            "base": "USD",
            "response_mode": "currency_list",
            "quotes": ["HKD", "SGD"],
            "rates_path": "items",
            "currency_field": "code",
            "rate_field": "mid"
        });
        let normalized = normalize_generic_fx_response(&payload, &config, "test").unwrap();
        let records = normalized_records(&normalized).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["quote_currency"], "HKD");
        assert_eq!(records[1]["quote_currency"], "SGD");
    }

    #[test]
    fn normalizes_fx_rates_from_per_currency_value_paths() {
        let payload = serde_json::json!({
            "payload": {
                "china": {"mid": "7.2351"},
                "europe": {"quote": 0.9234},
                "japan": [{"last": 158.2}]
            }
        });
        let config = serde_json::json!({
            "base": "USD",
            "response_mode": "currency_paths",
            "quotes": ["CNY", "EUR", "JPY"],
            "value_paths": {
                "CNY": "payload.china.mid",
                "EUR": "payload.europe.quote",
                "JPY": "payload.japan.0.last"
            }
        });
        let normalized = normalize_generic_fx_response(&payload, &config, "test").unwrap();
        let records = normalized_records(&normalized).unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0]["rate"], "7.2351");
        assert_eq!(records[1]["quote_currency"], "EUR");
        assert_eq!(records[2]["rate"], "158.2");
    }

    #[test]
    fn normalizes_multiple_coingecko_assets() {
        let payload = serde_json::json!({
            "bitcoin": {"usd": 68000},
            "tether": {"usd": "1.0002"}
        });
        let assets = vec![
            PriceAssetMapping {
                instrument_id: Uuid::now_v7().to_string(),
                lookup_key: "bitcoin".to_owned(),
            },
            PriceAssetMapping {
                instrument_id: Uuid::now_v7().to_string(),
                lookup_key: "tether".to_owned(),
            },
        ];
        let records = normalize_coingecko_response(&payload, &assets, "usd", "test").unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["price"], "68000");
        assert_eq!(records[1]["currency"], "USD");
    }

    #[test]
    fn normalizes_multiple_prices_from_asset_map() {
        let bitcoin_id = Uuid::now_v7().to_string();
        let usdt_id = Uuid::now_v7().to_string();
        let payload = serde_json::json!({
            "data": {"BTC": {"last": 68000}, "USDT": {"last": "1.0001"}}
        });
        let config = serde_json::json!({
            "response_mode": "asset_map",
            "currency": "USD",
            "assets": [
                {"instrument_id": bitcoin_id, "lookup_key": "BTC"},
                {"instrument_id": usdt_id, "lookup_key": "USDT"}
            ],
            "prices_path": "data",
            "price_field": "last"
        });
        let normalized = normalize_generic_price_response(&payload, &config, "test").unwrap();
        let records = normalized_records(&normalized).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["instrument_id"], bitcoin_id);
        assert_eq!(records[1]["price"], "1.0001");
    }

    #[test]
    fn normalizes_multiple_prices_from_asset_list() {
        let ethereum_id = Uuid::now_v7().to_string();
        let usdc_id = Uuid::now_v7().to_string();
        let payload = serde_json::json!({
            "items": [
                {"symbol": "eth", "quote": 3500.5},
                {"symbol": "USDC", "quote": 1}
            ]
        });
        let config = serde_json::json!({
            "response_mode": "asset_list",
            "currency": "USD",
            "assets": [
                {"instrument_id": ethereum_id, "lookup_key": "ETH"},
                {"instrument_id": usdc_id, "lookup_key": "USDC"}
            ],
            "prices_path": "items",
            "symbol_field": "symbol",
            "price_field": "quote"
        });
        let normalized = normalize_generic_price_response(&payload, &config, "test").unwrap();
        let records = normalized_records(&normalized).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["price"], "3500.5");
        assert_eq!(records[1]["instrument_id"], usdc_id);
    }

    #[test]
    fn rejects_multi_currency_collectors_without_quote_currencies() {
        let config = serde_json::json!({
            "provider": "generic_json",
            "url": "https://api.example.com/latest?base={base}&symbols={quotes}",
            "base": "USD",
            "response_mode": "currency_map",
            "quotes": [],
            "rates_path": "rates"
        });
        let error = validate_collector_config("fx_rates", &config).unwrap_err();
        assert!(error.to_string().contains("至少要选择一个报价币种"));
    }

    #[tokio::test]
    async fn automatically_creates_idempotent_us_and_hong_kong_collectors() {
        let pool = db::connect("sqlite::memory:").await.unwrap();
        let state = AppState {
            db: pool,
            auth_required: false,
        };
        for (symbol, name, asset_type, currency) in [
            ("AAPL", "Apple", "stock", "USD"),
            ("700.HK", "Tencent", "stock", "HKD"),
            ("CNY", "Renminbi", "cash", "CNY"),
        ] {
            sqlx::query(
                "INSERT INTO instruments(id,symbol,name,asset_type,currency) VALUES(?,?,?,?,?)",
            )
            .bind(Uuid::now_v7().to_string())
            .bind(symbol)
            .bind(name)
            .bind(asset_type)
            .bind(currency)
            .execute(&state.db)
            .await
            .unwrap();
        }

        ensure_stock_quote_collectors(&state).await.unwrap();
        ensure_stock_quote_collectors(&state).await.unwrap();

        let configs =
            sqlx::query_scalar::<_, String>("SELECT config_json FROM data_sources ORDER BY name")
                .fetch_all(&state.db)
                .await
                .unwrap();
        assert_eq!(configs.len(), 2);
        let symbols: Vec<String> = configs
            .into_iter()
            .map(|config| {
                serde_json::from_str::<Value>(&config).unwrap()["quote_symbol"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        assert!(symbols.contains(&"usAAPL".to_owned()));
        assert!(symbols.contains(&"hk00700".to_owned()));
        let intervals = sqlx::query_scalar::<_, i64>("SELECT interval_seconds FROM sync_jobs")
            .fetch_all(&state.db)
            .await
            .unwrap();
        assert_eq!(intervals, vec![300, 300]);
    }
}
