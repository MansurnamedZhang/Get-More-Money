use super::{AppState, common};
use crate::error::{AppError, AppResult};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
struct Classification {
    id: String,
    instrument_id: String,
    dimension: String,
    value: String,
    valid_from: String,
    created_at: String,
}
#[derive(Debug, Deserialize)]
struct ClassificationInput {
    instrument_id: String,
    dimension: String,
    value: String,
    valid_from: String,
}
#[derive(Debug, Serialize, sqlx::FromRow)]
struct Thesis {
    id: String,
    instrument_id: String,
    thesis: String,
    risks: String,
    invalidation: String,
    review_at: Option<String>,
    created_at: String,
    updated_at: String,
}
#[derive(Debug, Deserialize)]
struct ThesisInput {
    instrument_id: String,
    thesis: String,
    #[serde(default)]
    risks: String,
    #[serde(default)]
    invalidation: String,
    review_at: Option<String>,
}
#[derive(Debug, Serialize, sqlx::FromRow)]
struct Reconciliation {
    id: String,
    account_id: String,
    reconciled_at: String,
    statement_balance: String,
    ledger_balance: String,
    difference: String,
    note: String,
    created_at: String,
}
#[derive(Debug, Deserialize)]
struct ReconciliationInput {
    account_id: String,
    reconciled_at: String,
    statement_balance: String,
    #[serde(default)]
    note: String,
}
#[derive(Debug, Serialize, sqlx::FromRow)]
struct AuditLog {
    id: String,
    entity_type: String,
    entity_id: String,
    action: String,
    before_json: Option<String>,
    after_json: Option<String>,
    created_at: String,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/classifications",
            get(list_classifications).post(create_classification),
        )
        .route(
            "/classifications/{id}",
            get(get_classification).delete(delete_classification),
        )
        .route("/theses", get(list_theses).post(create_thesis))
        .route(
            "/theses/{id}",
            get(get_thesis).put(update_thesis).delete(delete_thesis),
        )
        .route(
            "/reconciliations",
            get(list_reconciliations).post(create_reconciliation),
        )
        .route("/audit-logs", get(list_audit_logs))
}

async fn list_classifications(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<Classification>>> {
    Ok(Json(
        sqlx::query_as::<_, Classification>(
            "SELECT * FROM classifications ORDER BY valid_from DESC",
        )
        .fetch_all(&state.db)
        .await?,
    ))
}
async fn get_classification(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Classification>> {
    Ok(Json(
        sqlx::query_as::<_, Classification>("SELECT * FROM classifications WHERE id=?")
            .bind(common::id(&id, "分类 ID")?)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::NotFound("分类不存在".to_owned()))?,
    ))
}
async fn create_classification(
    State(state): State<AppState>,
    Json(input): Json<ClassificationInput>,
) -> AppResult<(StatusCode, Json<Classification>)> {
    let id = Uuid::now_v7().to_string();
    let instrument = common::id(&input.instrument_id, "标的 ID")?;
    let dimension = common::required_text(&input.dimension, "分类维度", 80)?;
    let value = common::required_text(&input.value, "分类值", 160)?;
    let valid = common::rfc3339(&input.valid_from, "生效时间")?;
    sqlx::query("INSERT INTO classifications(id,instrument_id,dimension,value,valid_from) VALUES(?,?,?,?,?)").bind(&id).bind(instrument).bind(dimension).bind(value).bind(valid).execute(&state.db).await?;
    Ok((
        StatusCode::CREATED,
        Json(
            sqlx::query_as::<_, Classification>("SELECT * FROM classifications WHERE id=?")
                .bind(id)
                .fetch_one(&state.db)
                .await?,
        ),
    ))
}
async fn delete_classification(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let r = sqlx::query("DELETE FROM classifications WHERE id=?")
        .bind(common::id(&id, "分类 ID")?)
        .execute(&state.db)
        .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("分类不存在".to_owned()));
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn list_theses(State(state): State<AppState>) -> AppResult<Json<Vec<Thesis>>> {
    Ok(Json(
        sqlx::query_as::<_, Thesis>(
            "SELECT * FROM investment_theses ORDER BY review_at IS NULL,review_at",
        )
        .fetch_all(&state.db)
        .await?,
    ))
}
async fn get_thesis(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Thesis>> {
    Ok(Json(
        fetch_thesis(&state, &common::id(&id, "投资逻辑 ID")?).await?,
    ))
}
async fn create_thesis(
    State(state): State<AppState>,
    Json(input): Json<ThesisInput>,
) -> AppResult<(StatusCode, Json<Thesis>)> {
    let id = Uuid::now_v7().to_string();
    let v = validate_thesis(input)?;
    sqlx::query("INSERT INTO investment_theses(id,instrument_id,thesis,risks,invalidation,review_at) VALUES(?,?,?,?,?,?)").bind(&id).bind(v.0).bind(v.1).bind(v.2).bind(v.3).bind(v.4).execute(&state.db).await?;
    Ok((StatusCode::CREATED, Json(fetch_thesis(&state, &id).await?)))
}
async fn update_thesis(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<ThesisInput>,
) -> AppResult<Json<Thesis>> {
    let id = common::id(&id, "投资逻辑 ID")?;
    let v = validate_thesis(input)?;
    let r=sqlx::query("UPDATE investment_theses SET instrument_id=?,thesis=?,risks=?,invalidation=?,review_at=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?").bind(v.0).bind(v.1).bind(v.2).bind(v.3).bind(v.4).bind(&id).execute(&state.db).await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("投资逻辑不存在".to_owned()));
    }
    Ok(Json(fetch_thesis(&state, &id).await?))
}
async fn delete_thesis(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let r = sqlx::query("DELETE FROM investment_theses WHERE id=?")
        .bind(common::id(&id, "投资逻辑 ID")?)
        .execute(&state.db)
        .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("投资逻辑不存在".to_owned()));
    }
    Ok(StatusCode::NO_CONTENT)
}
fn validate_thesis(
    input: ThesisInput,
) -> AppResult<(String, String, String, String, Option<String>)> {
    Ok((
        common::id(&input.instrument_id, "标的 ID")?,
        common::required_text(&input.thesis, "投资逻辑", 6000)?,
        input.risks.trim().to_owned(),
        input.invalidation.trim().to_owned(),
        input
            .review_at
            .map(|v| common::rfc3339(&v, "复盘时间"))
            .transpose()?,
    ))
}
async fn fetch_thesis(state: &AppState, id: &str) -> AppResult<Thesis> {
    sqlx::query_as::<_, Thesis>("SELECT * FROM investment_theses WHERE id=?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("投资逻辑不存在".to_owned()))
}

async fn list_reconciliations(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<Reconciliation>>> {
    Ok(Json(
        sqlx::query_as::<_, Reconciliation>(
            "SELECT * FROM reconciliations ORDER BY reconciled_at DESC LIMIT 200",
        )
        .fetch_all(&state.db)
        .await?,
    ))
}
async fn create_reconciliation(
    State(state): State<AppState>,
    Json(input): Json<ReconciliationInput>,
) -> AppResult<(StatusCode, Json<Reconciliation>)> {
    let account = common::id(&input.account_id, "账户 ID")?;
    let at = common::rfc3339(&input.reconciled_at, "对账时间")?;
    let statement = common::decimal(&input.statement_balance, "对账单余额", true)?;
    let ledger = ledger_balance(&state, &account).await?;
    let difference = (Decimal::from_str(&statement).unwrap() - Decimal::from_str(&ledger).unwrap())
        .normalize()
        .to_string();
    let id = Uuid::now_v7().to_string();
    sqlx::query("INSERT INTO reconciliations(id,account_id,reconciled_at,statement_balance,ledger_balance,difference,note) VALUES(?,?,?,?,?,?,?)").bind(&id).bind(account).bind(at).bind(statement).bind(ledger).bind(difference).bind(input.note.trim()).execute(&state.db).await?;
    Ok((
        StatusCode::CREATED,
        Json(
            sqlx::query_as::<_, Reconciliation>("SELECT * FROM reconciliations WHERE id=?")
                .bind(id)
                .fetch_one(&state.db)
                .await?,
        ),
    ))
}
async fn ledger_balance(state: &AppState, account: &str) -> AppResult<String> {
    let values=sqlx::query_scalar::<_,String>(r#"SELECT l.quantity FROM transaction_legs l JOIN transactions t ON t.id=l.transaction_id JOIN instruments i ON i.id=l.instrument_id WHERE l.account_id=? AND t.status='confirmed' AND t.reverses_transaction_id IS NULL AND i.asset_type IN ('cash','stablecoin')"#).bind(account).fetch_all(&state.db).await?;
    Ok(values
        .into_iter()
        .map(|v| Decimal::from_str(&v).unwrap_or(Decimal::ZERO))
        .sum::<Decimal>()
        .normalize()
        .to_string())
}
async fn list_audit_logs(State(state): State<AppState>) -> AppResult<Json<Vec<AuditLog>>> {
    Ok(Json(
        sqlx::query_as::<_, AuditLog>(
            "SELECT * FROM audit_logs ORDER BY created_at DESC LIMIT 500",
        )
        .fetch_all(&state.db)
        .await?,
    ))
}
