use super::{AppState, common, settings};
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
struct Policy {
    objective: String,
    horizon_years: i64,
    max_drawdown: String,
    max_single_position: String,
    max_high_risk: String,
    emergency_fund_months: i64,
    allowed_tools: String,
    prohibited_tools: String,
    rebalance_frequency: String,
    review_frequency: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct PolicyInput {
    objective: String,
    horizon_years: i64,
    max_drawdown: String,
    max_single_position: String,
    max_high_risk: String,
    emergency_fund_months: i64,
    allowed_tools: String,
    prohibited_tools: String,
    rebalance_frequency: String,
    review_frequency: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct Target {
    id: String,
    dimension: String,
    value: String,
    target_weight: String,
    min_weight: String,
    max_weight: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct TargetInput {
    dimension: String,
    value: String,
    target_weight: String,
    min_weight: String,
    max_weight: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct Decision {
    id: String,
    instrument_id: Option<String>,
    action: String,
    decided_at: String,
    rationale: String,
    confidence: i64,
    risks: String,
    invalidation: String,
    review_at: Option<String>,
    outcome: String,
    process_score: Option<i64>,
    result_score: Option<i64>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct DecisionInput {
    instrument_id: Option<String>,
    action: String,
    decided_at: String,
    rationale: String,
    #[serde(default = "default_confidence")]
    confidence: i64,
    #[serde(default)]
    risks: String,
    #[serde(default)]
    invalidation: String,
    review_at: Option<String>,
    #[serde(default)]
    outcome: String,
    process_score: Option<i64>,
    result_score: Option<i64>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct Review {
    id: String,
    period_type: String,
    period_start: String,
    period_end: String,
    summary: String,
    actions: String,
    completed_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct ReviewInput {
    period_type: String,
    period_start: String,
    period_end: String,
    summary: String,
    #[serde(default)]
    actions: String,
    completed_at: Option<String>,
}

fn default_confidence() -> i64 {
    50
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/policy", get(get_policy).put(update_policy))
        .route("/targets", get(list_targets).post(create_target))
        .route(
            "/targets/{id}",
            get(get_target).put(update_target).delete(delete_target),
        )
        .route("/decisions", get(list_decisions).post(create_decision))
        .route(
            "/decisions/{id}",
            get(get_decision)
                .put(update_decision)
                .delete(delete_decision),
        )
        .route("/reviews", get(list_reviews).post(create_review))
        .route(
            "/reviews/{id}",
            get(get_review).put(update_review).delete(delete_review),
        )
}

async fn get_policy(State(state): State<AppState>) -> AppResult<Json<Policy>> {
    Ok(Json(fetch_policy(&state).await?))
}

async fn update_policy(
    State(state): State<AppState>,
    Json(input): Json<PolicyInput>,
) -> AppResult<Json<Policy>> {
    if !(1..=100).contains(&input.horizon_years)
        || !(0..=120).contains(&input.emergency_fund_months)
    {
        return Err(AppError::Validation(
            "投资期限或应急资金月数超出范围".to_owned(),
        ));
    }
    let objective = common::required_text(&input.objective, "投资目标", 1_000)?;
    let max_drawdown = settings::weight(&input.max_drawdown, "最大回撤")?;
    let max_single = settings::weight(&input.max_single_position, "单一标的上限")?;
    let max_risk = settings::weight(&input.max_high_risk, "高风险资产上限")?;
    sqlx::query(
        r#"UPDATE investment_policy SET objective=?, horizon_years=?, max_drawdown=?,
           max_single_position=?, max_high_risk=?, emergency_fund_months=?, allowed_tools=?,
           prohibited_tools=?, rebalance_frequency=?, review_frequency=?,
           updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=1"#,
    )
    .bind(objective)
    .bind(input.horizon_years)
    .bind(max_drawdown)
    .bind(max_single)
    .bind(max_risk)
    .bind(input.emergency_fund_months)
    .bind(common::required_text(
        &input.allowed_tools,
        "允许工具",
        1_000,
    )?)
    .bind(common::required_text(
        &input.prohibited_tools,
        "禁止工具",
        1_000,
    )?)
    .bind(common::required_text(
        &input.rebalance_frequency,
        "再平衡频率",
        40,
    )?)
    .bind(common::required_text(
        &input.review_frequency,
        "复盘频率",
        40,
    )?)
    .execute(&state.db)
    .await?;
    Ok(Json(fetch_policy(&state).await?))
}

async fn fetch_policy(state: &AppState) -> AppResult<Policy> {
    Ok(sqlx::query_as::<_, Policy>("SELECT objective,horizon_years,max_drawdown,max_single_position,max_high_risk,emergency_fund_months,allowed_tools,prohibited_tools,rebalance_frequency,review_frequency,updated_at FROM investment_policy WHERE id=1").fetch_one(&state.db).await?)
}

async fn list_targets(State(state): State<AppState>) -> AppResult<Json<Vec<Target>>> {
    Ok(Json(
        sqlx::query_as::<_, Target>("SELECT * FROM targets ORDER BY dimension,value")
            .fetch_all(&state.db)
            .await?,
    ))
}

async fn get_target(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Target>> {
    Ok(Json(
        fetch_target(&state, &common::id(&id, "目标 ID")?).await?,
    ))
}

async fn create_target(
    State(state): State<AppState>,
    Json(input): Json<TargetInput>,
) -> AppResult<(StatusCode, Json<Target>)> {
    let id = Uuid::now_v7().to_string();
    let values = validate_target(input)?;
    sqlx::query("INSERT INTO targets(id,dimension,value,target_weight,min_weight,max_weight) VALUES(?,?,?,?,?,?)")
        .bind(&id).bind(values.0).bind(values.1).bind(values.2).bind(values.3).bind(values.4)
        .execute(&state.db).await.map_err(map_unique)?;
    Ok((StatusCode::CREATED, Json(fetch_target(&state, &id).await?)))
}

async fn update_target(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<TargetInput>,
) -> AppResult<Json<Target>> {
    let id = common::id(&id, "目标 ID")?;
    let values = validate_target(input)?;
    let result = sqlx::query("UPDATE targets SET dimension=?,value=?,target_weight=?,min_weight=?,max_weight=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?")
        .bind(values.0).bind(values.1).bind(values.2).bind(values.3).bind(values.4).bind(&id)
        .execute(&state.db).await.map_err(map_unique)?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("配置目标不存在".to_owned()));
    }
    Ok(Json(fetch_target(&state, &id).await?))
}

async fn delete_target(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let id = common::id(&id, "目标 ID")?;
    let mut transaction = state.db.begin().await?;
    let target = sqlx::query_as::<_, Target>("SELECT * FROM targets WHERE id=?")
        .bind(&id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| AppError::NotFound("配置目标不存在".to_owned()))?;
    let result = sqlx::query("DELETE FROM targets WHERE id=?")
        .bind(&id)
        .execute(&mut *transaction)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("配置目标不存在".to_owned()));
    }
    sqlx::query(
        r#"INSERT INTO audit_logs(id, entity_type, entity_id, action, before_json)
           VALUES(?, 'target', ?, 'delete', ?)"#,
    )
    .bind(Uuid::now_v7().to_string())
    .bind(&id)
    .bind(serde_json::to_string(&target).expect("target is serializable"))
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn fetch_target(state: &AppState, id: &str) -> AppResult<Target> {
    sqlx::query_as::<_, Target>("SELECT * FROM targets WHERE id=?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("配置目标不存在".to_owned()))
}

fn validate_target(input: TargetInput) -> AppResult<(String, String, String, String, String)> {
    let dimension = common::required_text(&input.dimension, "配置维度", 80)?;
    let value = common::required_text(&input.value, "配置值", 120)?;
    let target = zero_weight(&input.target_weight, "目标权重")?;
    let min = zero_weight(&input.min_weight, "最小权重")?;
    let max = zero_weight(&input.max_weight, "最大权重")?;
    let target_d = Decimal::from_str(&target).unwrap();
    let min_d = Decimal::from_str(&min).unwrap();
    let max_d = Decimal::from_str(&max).unwrap();
    if min_d > target_d || target_d > max_d {
        return Err(AppError::Validation(
            "权重必须满足最小值 ≤ 目标值 ≤ 最大值".to_owned(),
        ));
    }
    Ok((dimension, value, target, min, max))
}

async fn list_decisions(State(state): State<AppState>) -> AppResult<Json<Vec<Decision>>> {
    Ok(Json(
        sqlx::query_as::<_, Decision>(
            "SELECT * FROM decision_logs ORDER BY decided_at DESC,id DESC",
        )
        .fetch_all(&state.db)
        .await?,
    ))
}
async fn get_decision(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Decision>> {
    Ok(Json(
        fetch_decision(&state, &common::id(&id, "决策 ID")?).await?,
    ))
}
async fn create_decision(
    State(state): State<AppState>,
    Json(input): Json<DecisionInput>,
) -> AppResult<(StatusCode, Json<Decision>)> {
    let id = Uuid::now_v7().to_string();
    let v = validate_decision(input)?;
    insert_decision(&state, &id, v).await?;
    Ok((
        StatusCode::CREATED,
        Json(fetch_decision(&state, &id).await?),
    ))
}
async fn update_decision(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<DecisionInput>,
) -> AppResult<Json<Decision>> {
    let id = common::id(&id, "决策 ID")?;
    let v = validate_decision(input)?;
    let r=sqlx::query("UPDATE decision_logs SET instrument_id=?,action=?,decided_at=?,rationale=?,confidence=?,risks=?,invalidation=?,review_at=?,outcome=?,process_score=?,result_score=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?").bind(v.0).bind(v.1).bind(v.2).bind(v.3).bind(v.4).bind(v.5).bind(v.6).bind(v.7).bind(v.8).bind(v.9).bind(v.10).bind(&id).execute(&state.db).await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("决策记录不存在".to_owned()));
    }
    Ok(Json(fetch_decision(&state, &id).await?))
}
async fn delete_decision(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let id = common::id(&id, "决策 ID")?;
    let mut transaction = state.db.begin().await?;
    let decision = sqlx::query_as::<_, Decision>("SELECT * FROM decision_logs WHERE id=?")
        .bind(&id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| AppError::NotFound("决策记录不存在".to_owned()))?;
    let r = sqlx::query("DELETE FROM decision_logs WHERE id=?")
        .bind(&id)
        .execute(&mut *transaction)
        .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("决策记录不存在".to_owned()));
    }
    sqlx::query(
        r#"INSERT INTO audit_logs(id, entity_type, entity_id, action, before_json)
           VALUES(?, 'decision', ?, 'delete', ?)"#,
    )
    .bind(Uuid::now_v7().to_string())
    .bind(&id)
    .bind(serde_json::to_string(&decision).expect("decision is serializable"))
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

type DecisionValues = (
    Option<String>,
    String,
    String,
    String,
    i64,
    String,
    String,
    Option<String>,
    String,
    Option<i64>,
    Option<i64>,
);
fn validate_decision(input: DecisionInput) -> AppResult<DecisionValues> {
    if !(0..=100).contains(&input.confidence)
        || input.process_score.is_some_and(|v| !(0..=100).contains(&v))
        || input.result_score.is_some_and(|v| !(0..=100).contains(&v))
    {
        return Err(AppError::Validation("评分必须在 0 到 100 之间".to_owned()));
    }
    Ok((
        input
            .instrument_id
            .map(|id| common::id(&id, "标的 ID"))
            .transpose()?,
        common::required_text(&input.action, "决策动作", 80)?,
        common::rfc3339(&input.decided_at, "决策时间")?,
        common::required_text(&input.rationale, "决策理由", 4000)?,
        input.confidence,
        input.risks.trim().to_owned(),
        input.invalidation.trim().to_owned(),
        input
            .review_at
            .map(|v| common::rfc3339(&v, "复盘时间"))
            .transpose()?,
        input.outcome.trim().to_owned(),
        input.process_score,
        input.result_score,
    ))
}
async fn insert_decision(state: &AppState, id: &str, v: DecisionValues) -> AppResult<()> {
    sqlx::query("INSERT INTO decision_logs(id,instrument_id,action,decided_at,rationale,confidence,risks,invalidation,review_at,outcome,process_score,result_score) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)").bind(id).bind(v.0).bind(v.1).bind(v.2).bind(v.3).bind(v.4).bind(v.5).bind(v.6).bind(v.7).bind(v.8).bind(v.9).bind(v.10).execute(&state.db).await?;
    Ok(())
}
async fn fetch_decision(state: &AppState, id: &str) -> AppResult<Decision> {
    sqlx::query_as::<_, Decision>("SELECT * FROM decision_logs WHERE id=?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("决策记录不存在".to_owned()))
}

async fn list_reviews(State(state): State<AppState>) -> AppResult<Json<Vec<Review>>> {
    Ok(Json(
        sqlx::query_as::<_, Review>("SELECT * FROM reviews ORDER BY period_end DESC,id DESC")
            .fetch_all(&state.db)
            .await?,
    ))
}
async fn get_review(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Review>> {
    Ok(Json(
        fetch_review(&state, &common::id(&id, "复盘 ID")?).await?,
    ))
}
async fn create_review(
    State(state): State<AppState>,
    Json(input): Json<ReviewInput>,
) -> AppResult<(StatusCode, Json<Review>)> {
    let id = Uuid::now_v7().to_string();
    let v = validate_review(input)?;
    sqlx::query("INSERT INTO reviews(id,period_type,period_start,period_end,summary,actions,completed_at) VALUES(?,?,?,?,?,?,?)").bind(&id).bind(v.0).bind(v.1).bind(v.2).bind(v.3).bind(v.4).bind(v.5).execute(&state.db).await?;
    Ok((StatusCode::CREATED, Json(fetch_review(&state, &id).await?)))
}
async fn update_review(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<ReviewInput>,
) -> AppResult<Json<Review>> {
    let id = common::id(&id, "复盘 ID")?;
    let v = validate_review(input)?;
    let r=sqlx::query("UPDATE reviews SET period_type=?,period_start=?,period_end=?,summary=?,actions=?,completed_at=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?").bind(v.0).bind(v.1).bind(v.2).bind(v.3).bind(v.4).bind(v.5).bind(&id).execute(&state.db).await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("复盘记录不存在".to_owned()));
    }
    Ok(Json(fetch_review(&state, &id).await?))
}
async fn delete_review(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let r = sqlx::query("DELETE FROM reviews WHERE id=?")
        .bind(common::id(&id, "复盘 ID")?)
        .execute(&state.db)
        .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("复盘记录不存在".to_owned()));
    }
    Ok(StatusCode::NO_CONTENT)
}
fn validate_review(
    input: ReviewInput,
) -> AppResult<(String, String, String, String, String, Option<String>)> {
    if !matches!(
        input.period_type.as_str(),
        "weekly" | "monthly" | "quarterly" | "annual"
    ) {
        return Err(AppError::Validation("复盘类型无效".to_owned()));
    }
    let start = common::required_text(&input.period_start, "开始日期", 10)?;
    let end = common::required_text(&input.period_end, "结束日期", 10)?;
    if start > end {
        return Err(AppError::Validation("开始日期不能晚于结束日期".to_owned()));
    }
    Ok((
        input.period_type,
        start,
        end,
        common::required_text(&input.summary, "复盘总结", 6000)?,
        input.actions.trim().to_owned(),
        input
            .completed_at
            .map(|v| common::rfc3339(&v, "完成时间"))
            .transpose()?,
    ))
}
async fn fetch_review(state: &AppState, id: &str) -> AppResult<Review> {
    sqlx::query_as::<_, Review>("SELECT * FROM reviews WHERE id=?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("复盘记录不存在".to_owned()))
}

fn zero_weight(value: &str, field: &str) -> AppResult<String> {
    let v = common::decimal(value, field, true)?;
    let d = Decimal::from_str(&v).unwrap();
    if d < Decimal::ZERO || d > Decimal::ONE {
        return Err(AppError::Validation(format!("{field} 必须在 0 到 1 之间")));
    }
    Ok(v)
}
fn map_unique(error: sqlx::Error) -> AppError {
    if common::is_unique_violation(&error) {
        AppError::Conflict("相同维度和值的配置目标已存在".to_owned())
    } else {
        error.into()
    }
}
