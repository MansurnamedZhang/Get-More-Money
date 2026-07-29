use super::{AppState, common, transactions};
use crate::{
    domain::{LegType, TransactionType},
    error::{AppError, AppResult},
};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{Query, State},
    routing::post,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct ImportQuery {
    #[serde(default)]
    commit: bool,
    source: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct CsvLeg {
    transaction_group: String,
    transaction_type: String,
    trade_at: String,
    account_id: String,
    instrument_id: String,
    leg_type: String,
    quantity: String,
    #[serde(default)]
    unit_price: String,
    #[serde(default)]
    price_currency: String,
    #[serde(default)]
    memo: String,
    #[serde(default)]
    external_id: String,
}

#[derive(Debug, Serialize)]
struct ImportResult {
    batch_id: Option<String>,
    groups: usize,
    rows: usize,
    valid_groups: usize,
    duplicate_groups: usize,
    imported_groups: usize,
    errors: Vec<String>,
    committed: bool,
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/imports/transactions", post(import_transactions))
}

async fn import_transactions(
    State(state): State<AppState>,
    Query(query): Query<ImportQuery>,
    body: Bytes,
) -> AppResult<Json<ImportResult>> {
    if body.len() > 10 * 1024 * 1024 {
        return Err(AppError::Validation("CSV 文件不能超过 10 MB".to_owned()));
    }
    let source = common::required_text(query.source.as_deref().unwrap_or("csv"), "导入来源", 100)?;
    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(body.as_ref());
    let mut groups: BTreeMap<String, Vec<CsvLeg>> = BTreeMap::new();
    let mut errors = Vec::new();
    let mut rows = 0;
    for (index, record) in reader.deserialize::<CsvLeg>().enumerate() {
        rows += 1;
        match record {
            Ok(record) if !record.transaction_group.trim().is_empty() => {
                groups
                    .entry(record.transaction_group.clone())
                    .or_default()
                    .push(record);
            }
            Ok(_) => errors.push(format!("第 {} 行缺少 transaction_group", index + 2)),
            Err(error) => errors.push(format!("第 {} 行解析失败：{}", index + 2, error)),
        }
    }

    let mut valid = Vec::new();
    let mut duplicate_groups = 0;
    for (group, records) in &groups {
        match build_input(records, &source) {
            Ok(input) => {
                if let Some(external_id) = input.external_id.as_deref() {
                    let duplicate: bool = sqlx::query_scalar(
                        "SELECT EXISTS(SELECT 1 FROM transactions WHERE source=? AND external_id=?)",
                    )
                    .bind(&source)
                    .bind(external_id)
                    .fetch_one(&state.db)
                    .await?;
                    if duplicate {
                        duplicate_groups += 1;
                        continue;
                    }
                }
                match transactions::validate(input) {
                    Ok(values) => valid.push((group.clone(), values)),
                    Err(error) => errors.push(format!("分组 {group}：{error}")),
                }
            }
            Err(error) => errors.push(format!("分组 {group}：{error}")),
        }
    }

    if !query.commit {
        return Ok(Json(ImportResult {
            batch_id: None,
            groups: groups.len(),
            rows,
            valid_groups: valid.len(),
            duplicate_groups,
            imported_groups: 0,
            errors,
            committed: false,
        }));
    }
    if !errors.is_empty() {
        return Err(AppError::Validation(
            "CSV 预览仍有错误，请修正后再提交".to_owned(),
        ));
    }
    let checksum = Sha256::digest(&body)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let batch_id = Uuid::now_v7().to_string();
    let mut database_transaction = state.db.begin().await?;
    let mut imported = 0;
    for (_, values) in valid {
        let id = Uuid::now_v7().to_string();
        transactions::insert_validated_transaction(&mut database_transaction, &id, values).await?;
        imported += 1;
    }
    let stats = serde_json::json!({
        "rows": rows,
        "groups": groups.len(),
        "imported": imported,
        "duplicates": duplicate_groups
    });
    sqlx::query(
        "INSERT INTO import_batches(id,source,checksum,status,stats_json) VALUES(?,?,?,'confirmed',?)",
    )
    .bind(&batch_id)
    .bind(&source)
    .bind(checksum)
    .bind(stats.to_string())
    .execute(&mut *database_transaction)
    .await
    .map_err(|error| {
        if common::is_unique_violation(&error) {
            AppError::Conflict("相同 CSV 文件已经导入".to_owned())
        } else {
            error.into()
        }
    })?;
    database_transaction.commit().await?;
    Ok(Json(ImportResult {
        batch_id: Some(batch_id),
        groups: groups.len(),
        rows,
        valid_groups: imported,
        duplicate_groups,
        imported_groups: imported,
        errors,
        committed: true,
    }))
}

fn build_input(
    records: &[CsvLeg],
    source: &str,
) -> AppResult<transactions::CreateTransactionInput> {
    let first = records
        .first()
        .ok_or_else(|| AppError::Validation("空分组".to_owned()))?;
    if records.iter().any(|record| {
        record.transaction_type != first.transaction_type || record.trade_at != first.trade_at
    }) {
        return Err(AppError::Validation(
            "同一分组的 transaction_type 和 trade_at 必须一致".to_owned(),
        ));
    }
    let transaction_type: TransactionType =
        serde_json::from_value(serde_json::Value::String(first.transaction_type.clone()))
            .map_err(|_| AppError::Validation("transaction_type 无效".to_owned()))?;
    let legs = records
        .iter()
        .map(|record| {
            let leg_type: LegType =
                serde_json::from_value(serde_json::Value::String(record.leg_type.clone()))
                    .map_err(|_| {
                        AppError::Validation(format!("leg_type 无效：{}", record.leg_type))
                    })?;
            Ok(transactions::CreateTransactionLegInput {
                account_id: record.account_id.clone(),
                instrument_id: record.instrument_id.clone(),
                leg_type,
                quantity: record.quantity.clone(),
                unit_price: (!record.unit_price.is_empty()).then(|| record.unit_price.clone()),
                price_currency: (!record.price_currency.is_empty())
                    .then(|| record.price_currency.clone()),
                memo: None,
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    Ok(transactions::CreateTransactionInput {
        transaction_type,
        trade_at: first.trade_at.clone(),
        settle_at: None,
        source: Some(source.to_owned()),
        external_id: (!first.external_id.is_empty()).then(|| first.external_id.clone()),
        memo: (!first.memo.is_empty()).then(|| first.memo.clone()),
        legs,
    })
}
