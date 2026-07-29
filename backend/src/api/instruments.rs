use super::{AppState, common};
use crate::{
    domain::AssetType,
    error::{AppError, AppResult},
    models::Instrument,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const TAG_DIMENSION_PREFIX: &str = "tag:";

#[derive(Debug, Serialize, sqlx::FromRow)]
struct InstrumentTag {
    id: String,
    instrument_id: String,
    name: String,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct InstrumentTagsInput {
    tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct InstrumentInput {
    symbol: String,
    name: String,
    asset_type: AssetType,
    currency: String,
    exchange: Option<String>,
    network: Option<String>,
    contract_address: Option<String>,
    precision: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct InstrumentStatusInput {
    is_active: bool,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/instrument-tags", get(list_instrument_tags))
        .route(
            "/instruments",
            get(list_instruments).post(create_instrument),
        )
        .route(
            "/instruments/{id}/tags",
            get(list_tags_for_instrument).put(replace_instrument_tags),
        )
        .route(
            "/instruments/{id}",
            get(get_instrument)
                .put(replace_instrument)
                .patch(set_instrument_status)
                .delete(delete_instrument),
        )
}

async fn list_instrument_tags(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<InstrumentTag>>> {
    let tags = sqlx::query_as::<_, InstrumentTag>(
        r#"SELECT id, instrument_id, value AS name, created_at
           FROM classifications
           WHERE dimension LIKE 'tag:%'
           ORDER BY lower(value), instrument_id"#,
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(tags))
}

async fn list_tags_for_instrument(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<InstrumentTag>>> {
    let id = common::id(&id, "标的 ID")?;
    fetch_instrument(&state, &id).await?;
    let tags = sqlx::query_as::<_, InstrumentTag>(
        r#"SELECT id, instrument_id, value AS name, created_at
           FROM classifications
           WHERE instrument_id = ? AND dimension LIKE 'tag:%'
           ORDER BY lower(value)"#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(tags))
}

async fn replace_instrument_tags(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<InstrumentTagsInput>,
) -> AppResult<Json<Vec<InstrumentTag>>> {
    let id = common::id(&id, "标的 ID")?;
    let tags = validate_tags(input.tags)?;
    let mut transaction = state.db.begin().await?;

    let instrument_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM instruments WHERE id = ?)")
            .bind(&id)
            .fetch_one(&mut *transaction)
            .await?;
    if !instrument_exists {
        return Err(AppError::NotFound("标的不存在".to_owned()));
    }

    let before = sqlx::query_scalar::<_, String>(
        r#"SELECT value FROM classifications
           WHERE instrument_id = ? AND dimension LIKE 'tag:%'
           ORDER BY lower(value)"#,
    )
    .bind(&id)
    .fetch_all(&mut *transaction)
    .await?;

    sqlx::query("DELETE FROM classifications WHERE instrument_id = ? AND dimension LIKE 'tag:%'")
        .bind(&id)
        .execute(&mut *transaction)
        .await?;

    for tag in &tags {
        let dimension = format!("{TAG_DIMENSION_PREFIX}{}", tag.to_lowercase());
        sqlx::query(
            r#"INSERT INTO classifications(id, instrument_id, dimension, value, valid_from)
               VALUES(?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))"#,
        )
        .bind(Uuid::now_v7().to_string())
        .bind(&id)
        .bind(dimension)
        .bind(tag)
        .execute(&mut *transaction)
        .await?;
    }

    sqlx::query(
        r#"INSERT INTO audit_logs(id, entity_type, entity_id, action, before_json, after_json)
           VALUES(?, 'instrument_tags', ?, 'replace', ?, ?)"#,
    )
    .bind(Uuid::now_v7().to_string())
    .bind(&id)
    .bind(serde_json::to_string(&before).expect("tag list is serializable"))
    .bind(serde_json::to_string(&tags).expect("tag list is serializable"))
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    let saved = sqlx::query_as::<_, InstrumentTag>(
        r#"SELECT id, instrument_id, value AS name, created_at
           FROM classifications
           WHERE instrument_id = ? AND dimension LIKE 'tag:%'
           ORDER BY lower(value)"#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(saved))
}

fn validate_tags(values: Vec<String>) -> AppResult<Vec<String>> {
    if values.len() > 20 {
        return Err(AppError::Validation(
            "单个投资标的最多设置 20 个类别标签".to_owned(),
        ));
    }

    let mut tags = Vec::new();
    let mut normalized = Vec::new();
    for value in values {
        let tag = common::required_text(&value, "类别标签", 30)?;
        let key = tag.to_lowercase();
        if !normalized.contains(&key) {
            normalized.push(key);
            tags.push(tag);
        }
    }
    Ok(tags)
}

async fn list_instruments(State(state): State<AppState>) -> AppResult<Json<Vec<Instrument>>> {
    let instruments = sqlx::query_as::<_, Instrument>(
        "SELECT * FROM instruments ORDER BY is_active DESC, upper(symbol), id",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(instruments))
}

async fn get_instrument(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Instrument>> {
    Ok(Json(
        fetch_instrument(&state, &common::id(&id, "标的 ID")?).await?,
    ))
}

async fn create_instrument(
    State(state): State<AppState>,
    Json(input): Json<InstrumentInput>,
) -> AppResult<(StatusCode, Json<Instrument>)> {
    let values = validate(input)?;
    let id = Uuid::now_v7().to_string();

    let result = sqlx::query(
        r#"INSERT INTO instruments
           (id, symbol, name, asset_type, currency, exchange, network, contract_address, precision)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&id)
    .bind(values.symbol)
    .bind(values.name)
    .bind(values.asset_type.as_str())
    .bind(values.currency)
    .bind(values.exchange)
    .bind(values.network)
    .bind(values.contract_address)
    .bind(values.precision)
    .execute(&state.db)
    .await;

    if let Err(error) = result {
        if common::is_unique_violation(&error) {
            return Err(AppError::Conflict(
                "相同市场或链上标识的标的已存在".to_owned(),
            ));
        }
        return Err(error.into());
    }

    Ok((
        StatusCode::CREATED,
        Json(fetch_instrument(&state, &id).await?),
    ))
}

async fn replace_instrument(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<InstrumentInput>,
) -> AppResult<Json<Instrument>> {
    let id = common::id(&id, "标的 ID")?;
    let values = validate(input)?;

    let result = sqlx::query(
        r#"UPDATE instruments
           SET symbol = ?, name = ?, asset_type = ?, currency = ?, exchange = ?,
               network = ?, contract_address = ?, precision = ?,
               updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
           WHERE id = ?"#,
    )
    .bind(values.symbol)
    .bind(values.name)
    .bind(values.asset_type.as_str())
    .bind(values.currency)
    .bind(values.exchange)
    .bind(values.network)
    .bind(values.contract_address)
    .bind(values.precision)
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(result) if result.rows_affected() == 0 => {
            return Err(AppError::NotFound("标的不存在".to_owned()));
        }
        Err(error) if common::is_unique_violation(&error) => {
            return Err(AppError::Conflict(
                "相同市场或链上标识的标的已存在".to_owned(),
            ));
        }
        Err(error) => return Err(error.into()),
        Ok(_) => {}
    }

    Ok(Json(fetch_instrument(&state, &id).await?))
}

async fn set_instrument_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<InstrumentStatusInput>,
) -> AppResult<Json<Instrument>> {
    let id = common::id(&id, "标的 ID")?;
    let result = sqlx::query(
        r#"UPDATE instruments
           SET is_active = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
           WHERE id = ?"#,
    )
    .bind(input.is_active)
    .bind(&id)
    .execute(&state.db)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("标的不存在".to_owned()));
    }
    Ok(Json(fetch_instrument(&state, &id).await?))
}

async fn delete_instrument(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let id = common::id(&id, "标的 ID")?;
    let mut transaction = state.db.begin().await?;
    let used_in_ledger: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM transaction_legs WHERE instrument_id = ?)")
            .bind(&id)
            .fetch_one(&mut *transaction)
            .await?;
    if used_in_ledger {
        return Err(AppError::Conflict(
            "标的已有账本流水，不能永久删除；请改为停用以保留历史记录".to_owned(),
        ));
    }

    sqlx::query(
        r#"UPDATE sync_jobs
           SET is_enabled = 0, deleted_at = COALESCE(deleted_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
               updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
           WHERE data_source_id IN (
             SELECT id FROM data_sources WHERE json_extract(config_json, '$.instrument_id') = ?
           )"#,
    )
    .bind(&id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        r#"UPDATE data_sources
           SET is_enabled = 0, deleted_at = COALESCE(deleted_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
               updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
           WHERE json_extract(config_json, '$.instrument_id') = ?"#,
    )
    .bind(&id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query("DELETE FROM prices WHERE instrument_id = ?")
        .bind(&id)
        .execute(&mut *transaction)
        .await?;
    let result = sqlx::query("DELETE FROM instruments WHERE id = ?")
        .bind(&id)
        .execute(&mut *transaction)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("标的不存在".to_owned()));
    }
    transaction.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

struct ValidatedInstrument {
    symbol: String,
    name: String,
    asset_type: AssetType,
    currency: String,
    exchange: Option<String>,
    network: Option<String>,
    contract_address: Option<String>,
    precision: Option<i64>,
}

fn validate(input: InstrumentInput) -> AppResult<ValidatedInstrument> {
    if let Some(precision) = input.precision
        && !(0..=30).contains(&precision)
    {
        return Err(AppError::Validation("精度必须在 0 到 30 之间".to_owned()));
    }

    let symbol = common::required_text(&input.symbol, "标的代码", 32)?.to_ascii_uppercase();
    let network = normalize_networks(input.network.as_deref())?;
    let contract_address =
        common::optional_text(input.contract_address.as_deref(), "合约地址", 256)?;

    if contract_address.is_some() && network.is_none() {
        return Err(AppError::Validation(
            "填写合约地址时必须同时填写区块链网络".to_owned(),
        ));
    }

    Ok(ValidatedInstrument {
        symbol,
        name: common::required_text(&input.name, "标的名称", 160)?,
        asset_type: input.asset_type,
        currency: common::currency(&input.currency, "计价币种")?,
        exchange: common::optional_text(input.exchange.as_deref(), "交易市场", 64)?,
        network,
        contract_address,
        precision: input.precision,
    })
}

fn normalize_networks(value: Option<&str>) -> AppResult<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let mut networks = Vec::new();
    for raw in value.split([',', '，', '、']) {
        let code = raw
            .trim()
            .to_ascii_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("-");
        if code.is_empty() {
            continue;
        }
        if code.len() > 32
            || !code.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            })
        {
            return Err(AppError::Validation(
                "区块链网络代码只能包含字母、数字、连字符或下划线，且每项不超过 32 个字符"
                    .to_owned(),
            ));
        }
        if !networks.iter().any(|network| network == &code) {
            networks.push(code);
        }
    }
    if networks.len() > 16 {
        return Err(AppError::Validation(
            "单个标的最多选择 16 个区块链网络".to_owned(),
        ));
    }
    if networks.is_empty() {
        Ok(None)
    } else {
        Ok(Some(networks.join(",")))
    }
}

async fn fetch_instrument(state: &AppState, id: &str) -> AppResult<Instrument> {
    sqlx::query_as::<_, Instrument>("SELECT * FROM instruments WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("标的不存在".to_owned()))
}
