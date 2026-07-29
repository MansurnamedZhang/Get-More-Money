use super::{AppState, common};
use crate::{
    domain::{LegType, TransactionType},
    error::{AppError, AppResult},
    models::{TransactionLeg, TransactionRecord, TransactionWithLegs},
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Sqlite, Transaction};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub(crate) struct CreateTransactionInput {
    pub(crate) transaction_type: TransactionType,
    pub(crate) trade_at: String,
    pub(crate) settle_at: Option<String>,
    pub(crate) source: Option<String>,
    pub(crate) external_id: Option<String>,
    pub(crate) memo: Option<String>,
    pub(crate) legs: Vec<CreateTransactionLegInput>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateTransactionLegInput {
    pub(crate) account_id: String,
    pub(crate) instrument_id: String,
    pub(crate) leg_type: LegType,
    pub(crate) quantity: String,
    pub(crate) unit_price: Option<String>,
    pub(crate) price_currency: Option<String>,
    pub(crate) memo: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Pagination {
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Debug, Serialize)]
struct DuplicateCheckResult {
    duplicate: bool,
    matches: Vec<DuplicateMatch>,
}

#[derive(Debug, Serialize)]
struct DuplicateMatch {
    id: String,
    trade_at: String,
    memo: Option<String>,
    source: String,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/transactions",
            get(list_transactions).post(create_transaction),
        )
        .route("/transactions/duplicate-check", post(check_duplicate))
        .route(
            "/transactions/{id}/permanent",
            delete(permanently_delete_transaction),
        )
        .route(
            "/transactions/{id}",
            get(get_transaction)
                .put(replace_transaction)
                .delete(delete_transaction),
        )
}

async fn check_duplicate(
    State(state): State<AppState>,
    Json(input): Json<CreateTransactionInput>,
) -> AppResult<Json<DuplicateCheckResult>> {
    let values = validate(input)?;
    let candidates = sqlx::query_as::<_, TransactionRecord>(
        r#"SELECT * FROM transactions
           WHERE transaction_type = ? AND status = 'confirmed'
             AND reverses_transaction_id IS NULL
             AND julianday(trade_at) BETWEEN julianday(?) - 1 AND julianday(?) + 1
           ORDER BY trade_at DESC, id DESC LIMIT 20"#,
    )
    .bind(values.transaction_type.as_str())
    .bind(&values.trade_at)
    .bind(&values.trade_at)
    .fetch_all(&state.db)
    .await?;

    let expected_signature = validated_leg_signature(&values.legs);
    let mut matches = Vec::new();
    for candidate in candidates {
        let legs = fetch_legs(&state, &candidate.id).await?;
        if stored_leg_signature(&legs) == expected_signature {
            matches.push(DuplicateMatch {
                id: candidate.id,
                trade_at: candidate.trade_at,
                memo: candidate.memo,
                source: candidate.source,
            });
            if matches.len() == 5 {
                break;
            }
        }
    }

    Ok(Json(DuplicateCheckResult {
        duplicate: !matches.is_empty(),
        matches,
    }))
}

async fn list_transactions(
    State(state): State<AppState>,
    Query(page): Query<Pagination>,
) -> AppResult<Json<Vec<TransactionWithLegs>>> {
    let limit = page.limit.unwrap_or(50);
    let offset = page.offset.unwrap_or(0);
    if !(1..=200).contains(&limit) || offset < 0 {
        return Err(AppError::Validation(
            "limit 必须在 1 到 200 之间，offset 不能为负数".to_owned(),
        ));
    }

    let records = sqlx::query_as::<_, TransactionRecord>(
        r#"SELECT * FROM transactions
           WHERE status = 'confirmed' AND reverses_transaction_id IS NULL
           ORDER BY trade_at DESC, id DESC LIMIT ? OFFSET ?"#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    let mut output = Vec::with_capacity(records.len());
    for record in records {
        let legs = fetch_legs(&state, &record.id).await?;
        output.push(TransactionWithLegs {
            transaction: record,
            legs,
        });
    }
    Ok(Json(output))
}

async fn get_transaction(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<TransactionWithLegs>> {
    let id = common::id(&id, "交易 ID")?;
    Ok(Json(fetch_transaction(&state, &id).await?))
}

async fn create_transaction(
    State(state): State<AppState>,
    Json(input): Json<CreateTransactionInput>,
) -> AppResult<(StatusCode, Json<TransactionWithLegs>)> {
    let values = validate(input)?;
    let id = Uuid::now_v7().to_string();
    let mut database_transaction = state.db.begin().await?;
    insert_validated_transaction(&mut database_transaction, &id, values).await?;

    database_transaction.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(fetch_transaction(&state, &id).await?),
    ))
}

async fn replace_transaction(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<CreateTransactionInput>,
) -> AppResult<Json<TransactionWithLegs>> {
    let id = common::id(&id, "交易 ID")?;
    let replacement = validate(input)?;
    let replacement_id = Uuid::now_v7().to_string();
    let mut database_transaction = state.db.begin().await?;

    let original =
        sqlx::query_as::<_, TransactionRecord>("SELECT * FROM transactions WHERE id = ?")
            .bind(&id)
            .fetch_optional(&mut *database_transaction)
            .await?
            .ok_or_else(|| AppError::NotFound("交易不存在".to_owned()))?;

    if original.status != "confirmed" || original.reverses_transaction_id.is_some() {
        return Err(AppError::Conflict(
            "该流水已被冲销，不能再次编辑".to_owned(),
        ));
    }

    reverse_confirmed_transaction(
        &mut database_transaction,
        &original,
        "system_correction",
        format!("冲销流水 {}", original.id),
    )
    .await?;

    insert_validated_transaction(&mut database_transaction, &replacement_id, replacement).await?;
    database_transaction.commit().await?;

    Ok(Json(fetch_transaction(&state, &replacement_id).await?))
}

async fn delete_transaction(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let id = common::id(&id, "交易 ID")?;
    let mut database_transaction = state.db.begin().await?;
    let original =
        sqlx::query_as::<_, TransactionRecord>("SELECT * FROM transactions WHERE id = ?")
            .bind(&id)
            .fetch_optional(&mut *database_transaction)
            .await?
            .ok_or_else(|| AppError::NotFound("交易不存在".to_owned()))?;

    if original.status != "confirmed" || original.reverses_transaction_id.is_some() {
        return Err(AppError::Conflict(
            "该流水已被冲销或作废，不能再次删除".to_owned(),
        ));
    }

    reverse_confirmed_transaction(
        &mut database_transaction,
        &original,
        "system_void",
        format!("作废流水 {}", original.id),
    )
    .await?;
    database_transaction.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn permanently_delete_transaction(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let id = common::id(&id, "交易 ID")?;
    let mut database_transaction = state.db.begin().await?;
    let original =
        sqlx::query_as::<_, TransactionRecord>("SELECT * FROM transactions WHERE id = ?")
            .bind(&id)
            .fetch_optional(&mut *database_transaction)
            .await?
            .ok_or_else(|| AppError::NotFound("交易不存在".to_owned()))?;

    if original.status != "confirmed" || original.reverses_transaction_id.is_some() {
        return Err(AppError::Conflict(
            "该流水已被冲销或属于冲销记录，不能彻底删除".to_owned(),
        ));
    }
    if !matches!(original.source.as_str(), "manual" | "web" | "web_standard") {
        return Err(AppError::Conflict(
            "只有手工录入且尚未进入外部数据链路的流水可以彻底删除".to_owned(),
        ));
    }
    let linked_reversals: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM transactions WHERE reverses_transaction_id = ?")
            .bind(&id)
            .fetch_one(&mut *database_transaction)
            .await?;
    if linked_reversals > 0 {
        return Err(AppError::Conflict(
            "该流水已经产生更正或撤销关系，不能彻底删除".to_owned(),
        ));
    }

    let delete_minutes: i64 =
        sqlx::query_scalar("SELECT transaction_hard_delete_minutes FROM app_settings WHERE id = 1")
            .fetch_one(&mut *database_transaction)
            .await?;
    if delete_minutes == 0 {
        return Err(AppError::Conflict(
            "设置中已关闭流水彻底删除功能".to_owned(),
        ));
    }
    let created_at = DateTime::parse_from_rfc3339(&original.created_at)
        .map_err(|_| AppError::Conflict("流水创建时间无效，不能彻底删除".to_owned()))?
        .with_timezone(&Utc);
    let deadline = created_at + Duration::minutes(delete_minutes);
    if Utc::now() > deadline {
        return Err(AppError::Conflict(format!(
            "该流水已超过 {delete_minutes} 分钟纠错时限，请使用撤销功能"
        )));
    }

    sqlx::query("DELETE FROM transaction_legs WHERE transaction_id = ?")
        .bind(&id)
        .execute(&mut *database_transaction)
        .await?;
    sqlx::query("DELETE FROM transactions WHERE id = ?")
        .bind(&id)
        .execute(&mut *database_transaction)
        .await?;
    sqlx::query(
        r#"INSERT INTO audit_logs(id,entity_type,entity_id,action,before_json)
           VALUES(?,'transaction',?,'hard_delete',?)"#,
    )
    .bind(Uuid::now_v7().to_string())
    .bind(&id)
    .bind(
        json!({
            "transaction_type": original.transaction_type,
            "source": original.source,
            "created_at": original.created_at
        })
        .to_string(),
    )
    .execute(&mut *database_transaction)
    .await?;
    database_transaction.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn reverse_confirmed_transaction(
    database_transaction: &mut Transaction<'_, Sqlite>,
    original: &TransactionRecord,
    source: &str,
    memo: String,
) -> AppResult<String> {
    let reversal_id = Uuid::now_v7().to_string();
    let original_legs = sqlx::query_as::<_, TransactionLeg>(
        "SELECT * FROM transaction_legs WHERE transaction_id = ? ORDER BY sequence",
    )
    .bind(&original.id)
    .fetch_all(&mut **database_transaction)
    .await?;

    sqlx::query("UPDATE transactions SET status = 'reversed' WHERE id = ?")
        .bind(&original.id)
        .execute(&mut **database_transaction)
        .await?;

    sqlx::query(
        r#"INSERT INTO transactions
           (id, transaction_type, trade_at, settle_at, source, memo, status, reverses_transaction_id)
           VALUES (?, ?, ?, ?, ?, ?, 'confirmed', ?)"#,
    )
    .bind(&reversal_id)
    .bind(&original.transaction_type)
    .bind(&original.trade_at)
    .bind(&original.settle_at)
    .bind(source)
    .bind(memo)
    .bind(&original.id)
    .execute(&mut **database_transaction)
    .await?;

    for leg in original_legs {
        let quantity = Decimal::from_str(&leg.quantity)
            .expect("stored transaction quantity must be a decimal");
        sqlx::query(
            r#"INSERT INTO transaction_legs
               (id, transaction_id, sequence, account_id, instrument_id, leg_type,
                quantity, unit_price, price_currency, memo)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(Uuid::now_v7().to_string())
        .bind(&reversal_id)
        .bind(leg.sequence)
        .bind(leg.account_id)
        .bind(leg.instrument_id)
        .bind(leg.leg_type)
        .bind((-quantity).normalize().to_string())
        .bind(leg.unit_price)
        .bind(leg.price_currency)
        .bind(leg.memo)
        .execute(&mut **database_transaction)
        .await?;
    }

    Ok(reversal_id)
}

pub(crate) async fn insert_validated_transaction(
    database_transaction: &mut Transaction<'_, Sqlite>,
    id: &str,
    values: ValidatedTransaction,
) -> AppResult<()> {
    let insert = sqlx::query(
        r#"INSERT INTO transactions
           (id, transaction_type, trade_at, settle_at, source, external_id, memo)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(id)
    .bind(values.transaction_type.as_str())
    .bind(values.trade_at)
    .bind(values.settle_at)
    .bind(&values.source)
    .bind(&values.external_id)
    .bind(values.memo)
    .execute(&mut **database_transaction)
    .await;

    if let Err(error) = insert {
        if common::is_unique_violation(&error) {
            return Err(AppError::Conflict(
                "该数据源的外部流水号已经入账".to_owned(),
            ));
        }
        return Err(error.into());
    }

    for (sequence, leg) in values.legs.into_iter().enumerate() {
        ensure_reference_exists(database_transaction, &leg.account_id, &leg.instrument_id).await?;

        sqlx::query(
            r#"INSERT INTO transaction_legs
               (id, transaction_id, sequence, account_id, instrument_id, leg_type,
                quantity, unit_price, price_currency, memo)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(Uuid::now_v7().to_string())
        .bind(id)
        .bind(sequence as i64)
        .bind(leg.account_id)
        .bind(leg.instrument_id)
        .bind(leg.leg_type.as_str())
        .bind(leg.quantity)
        .bind(leg.unit_price)
        .bind(leg.price_currency)
        .bind(leg.memo)
        .execute(&mut **database_transaction)
        .await?;
    }
    Ok(())
}

pub(crate) struct ValidatedTransaction {
    transaction_type: TransactionType,
    trade_at: String,
    settle_at: Option<String>,
    source: String,
    external_id: Option<String>,
    memo: Option<String>,
    legs: Vec<ValidatedLeg>,
}

struct ValidatedLeg {
    account_id: String,
    instrument_id: String,
    leg_type: LegType,
    quantity: String,
    unit_price: Option<String>,
    price_currency: Option<String>,
    memo: Option<String>,
}

fn validated_leg_signature(legs: &[ValidatedLeg]) -> Vec<String> {
    let mut signature = legs
        .iter()
        .map(|leg| {
            format!(
                "{}|{}|{}|{}|{}|{}",
                leg.account_id,
                leg.instrument_id,
                leg.leg_type.as_str(),
                leg.quantity,
                leg.unit_price.as_deref().unwrap_or_default(),
                leg.price_currency.as_deref().unwrap_or_default()
            )
        })
        .collect::<Vec<_>>();
    signature.sort_unstable();
    signature
}

fn stored_leg_signature(legs: &[TransactionLeg]) -> Vec<String> {
    let mut signature = legs
        .iter()
        .map(|leg| {
            format!(
                "{}|{}|{}|{}|{}|{}",
                leg.account_id,
                leg.instrument_id,
                leg.leg_type,
                leg.quantity,
                leg.unit_price.as_deref().unwrap_or_default(),
                leg.price_currency.as_deref().unwrap_or_default()
            )
        })
        .collect::<Vec<_>>();
    signature.sort_unstable();
    signature
}

pub(crate) fn validate(input: CreateTransactionInput) -> AppResult<ValidatedTransaction> {
    if input.legs.is_empty() || input.legs.len() > 100 {
        return Err(AppError::Validation(
            "一笔交易必须包含 1 到 100 条分录".to_owned(),
        ));
    }

    let trade_at = common::rfc3339(&input.trade_at, "交易时间")?;
    let settle_at = input
        .settle_at
        .as_deref()
        .map(|value| common::rfc3339(value, "结算时间"))
        .transpose()?;

    if let Some(settle_at) = &settle_at
        && DateTime::parse_from_rfc3339(settle_at).expect("validated timestamp")
            < DateTime::parse_from_rfc3339(&trade_at).expect("validated timestamp")
    {
        return Err(AppError::Validation("结算时间不能早于交易时间".to_owned()));
    }

    let source =
        common::required_text(input.source.as_deref().unwrap_or("manual"), "数据来源", 100)?;
    let external_id = common::optional_text(input.external_id.as_deref(), "外部流水号", 160)?;
    let memo = common::optional_text(input.memo.as_deref(), "交易备注", 1000)?;

    let mut legs = Vec::with_capacity(input.legs.len());
    for leg in input.legs {
        let unit_price = leg
            .unit_price
            .as_deref()
            .map(|value| common::positive_decimal(value, "单价"))
            .transpose()?;
        let price_currency = leg
            .price_currency
            .as_deref()
            .map(|value| common::currency(value, "单价币种"))
            .transpose()?;

        if unit_price.is_some() != price_currency.is_some() {
            return Err(AppError::Validation(
                "单价和单价币种必须同时填写".to_owned(),
            ));
        }

        legs.push(ValidatedLeg {
            account_id: common::id(&leg.account_id, "分录账户 ID")?,
            instrument_id: common::id(&leg.instrument_id, "分录标的 ID")?,
            leg_type: leg.leg_type,
            quantity: common::decimal(&leg.quantity, "分录数量", false)?,
            unit_price,
            price_currency,
            memo: common::optional_text(leg.memo.as_deref(), "分录备注", 500)?,
        });
    }

    validate_leg_shape(input.transaction_type, &legs)?;

    Ok(ValidatedTransaction {
        transaction_type: input.transaction_type,
        trade_at,
        settle_at,
        source,
        external_id,
        memo,
        legs,
    })
}

fn validate_leg_shape(transaction_type: TransactionType, legs: &[ValidatedLeg]) -> AppResult<()> {
    let decimal_value = |leg: &ValidatedLeg| {
        Decimal::from_str(&leg.quantity).expect("validated decimal must parse")
    };
    let has_positive = legs.iter().any(|leg| decimal_value(leg) > Decimal::ZERO);
    let has_negative = legs.iter().any(|leg| decimal_value(leg) < Decimal::ZERO);

    match transaction_type {
        TransactionType::Buy | TransactionType::Sell => {
            let asset_legs: Vec<_> = legs
                .iter()
                .filter(|leg| leg.leg_type == LegType::Asset)
                .collect();
            let cash_legs: Vec<_> = legs
                .iter()
                .filter(|leg| leg.leg_type == LegType::Cash)
                .collect();
            if asset_legs.is_empty() || cash_legs.is_empty() {
                return Err(AppError::Validation(
                    "买卖交易必须同时包含资产分录和现金分录".to_owned(),
                ));
            }
            let buy = transaction_type == TransactionType::Buy;
            let directions_valid = asset_legs.iter().all(|leg| {
                let value = decimal_value(leg);
                if buy {
                    value > Decimal::ZERO
                } else {
                    value < Decimal::ZERO
                }
            }) && cash_legs.iter().all(|leg| {
                let value = decimal_value(leg);
                if buy {
                    value < Decimal::ZERO
                } else {
                    value > Decimal::ZERO
                }
            });
            if !directions_valid {
                return Err(AppError::Validation(
                    "买入应增加资产并减少现金，卖出应减少资产并增加现金".to_owned(),
                ));
            }
        }
        TransactionType::Transfer if legs.len() < 2 || !has_positive || !has_negative => {
            return Err(AppError::Validation(
                "内部转账至少需要一条转出分录和一条转入分录".to_owned(),
            ));
        }
        TransactionType::Deposit
        | TransactionType::Dividend
        | TransactionType::Interest
        | TransactionType::StakingReward
        | TransactionType::Airdrop
            if !has_positive =>
        {
            return Err(AppError::Validation("该交易必须包含正数分录".to_owned()));
        }
        TransactionType::Withdrawal | TransactionType::Fee | TransactionType::Tax
            if !has_negative =>
        {
            return Err(AppError::Validation("该交易必须包含负数分录".to_owned()));
        }
        _ => {}
    }
    Ok(())
}

async fn ensure_reference_exists(
    database_transaction: &mut Transaction<'_, Sqlite>,
    account_id: &str,
    instrument_id: &str,
) -> AppResult<()> {
    let account_status =
        sqlx::query_scalar::<_, bool>("SELECT is_active FROM accounts WHERE id = ?")
            .bind(account_id)
            .fetch_optional(&mut **database_transaction)
            .await?;
    let Some(account_is_active) = account_status else {
        return Err(AppError::Validation(format!(
            "分录账户不存在：{account_id}"
        )));
    };
    if !account_is_active {
        return Err(AppError::Validation(
            "冻结账户不能录入新流水；请先解冻账户".to_owned(),
        ));
    }

    let instrument_exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM instruments WHERE id = ?)")
            .bind(instrument_id)
            .fetch_one(&mut **database_transaction)
            .await?;
    if !instrument_exists {
        return Err(AppError::Validation(format!(
            "分录标的不存在：{instrument_id}"
        )));
    }
    Ok(())
}

async fn fetch_transaction(state: &AppState, id: &str) -> AppResult<TransactionWithLegs> {
    let transaction =
        sqlx::query_as::<_, TransactionRecord>("SELECT * FROM transactions WHERE id = ?")
            .bind(id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::NotFound("交易不存在".to_owned()))?;
    let legs = fetch_legs(state, id).await?;
    Ok(TransactionWithLegs { transaction, legs })
}

async fn fetch_legs(state: &AppState, transaction_id: &str) -> AppResult<Vec<TransactionLeg>> {
    Ok(sqlx::query_as::<_, TransactionLeg>(
        "SELECT * FROM transaction_legs WHERE transaction_id = ? ORDER BY sequence",
    )
    .bind(transaction_id)
    .fetch_all(&state.db)
    .await?)
}
