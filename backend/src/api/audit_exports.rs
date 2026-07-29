use super::{AppState, settings};
use crate::{
    error::{AppError, AppResult},
    models::{Account, BlockchainNetwork, Instrument},
};
use axum::{
    Json, Router,
    body::Body,
    extract::{Query, State},
    http::{Response, StatusCode, header},
    routing::get,
};
use chrono::{NaiveDate, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

const AUDIT_SCHEMA_VERSION: &str = "2026.07-audit-v1";

#[derive(Debug, Clone, Deserialize)]
struct AuditQuery {
    from: Option<String>,
    to: Option<String>,
    include_market: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
struct AuditPeriod {
    from: Option<String>,
    to: Option<String>,
}

#[derive(Debug, Clone)]
struct ValidatedPeriod {
    from: Option<String>,
    to: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct AuditCounts {
    accounts: i64,
    active_accounts: i64,
    instruments: i64,
    active_instruments: i64,
    transactions: i64,
    transaction_legs: i64,
    reversed_transactions: i64,
    prices: i64,
    fx_rates: i64,
    import_batches: i64,
    reconciliations: i64,
    audit_logs: i64,
    decisions: i64,
    reviews: i64,
    sync_runs: i64,
}

#[derive(Debug, Clone, Serialize)]
struct AuditCheck {
    code: &'static str,
    label: &'static str,
    level: &'static str,
    count: i64,
    detail: String,
}

#[derive(Debug, Clone, Serialize)]
struct AuditIntegrity {
    passed: bool,
    critical_issue_count: i64,
    warning_count: i64,
    checks: Vec<AuditCheck>,
}

#[derive(Debug, Clone, Serialize)]
struct AuditSummary {
    generated_at: String,
    schema_version: &'static str,
    period: AuditPeriod,
    report_currency: String,
    timezone: String,
    counts: AuditCounts,
    integrity: AuditIntegrity,
}

#[derive(Debug, Serialize)]
struct AuditManifest {
    export_id: String,
    generated_at: String,
    schema_version: &'static str,
    period: AuditPeriod,
    report_currency: String,
    timezone: String,
    includes_market_history: bool,
    data_sha256: String,
    note: &'static str,
}

#[derive(Debug, Serialize, FromRow)]
struct AuditTransactionRow {
    transaction_id: String,
    transaction_status: String,
    transaction_type: String,
    trade_at: String,
    settle_at: Option<String>,
    transaction_created_at: String,
    source: String,
    external_id: Option<String>,
    transaction_memo: Option<String>,
    reverses_transaction_id: Option<String>,
    leg_id: String,
    leg_sequence: i64,
    account_id: String,
    account_name: String,
    institution: Option<String>,
    instrument_id: String,
    instrument_symbol: String,
    instrument_name: String,
    asset_type: String,
    leg_type: String,
    quantity: String,
    unit_price: Option<String>,
    price_currency: Option<String>,
    leg_memo: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
struct AuditLogRow {
    id: String,
    entity_type: String,
    entity_id: String,
    action: String,
    before_json: Option<String>,
    after_json: Option<String>,
    created_at: String,
}

#[derive(Debug, Serialize, FromRow)]
struct ImportBatchRow {
    id: String,
    source: String,
    imported_at: String,
    checksum: String,
    status: String,
    stats_json: String,
}

#[derive(Debug, Serialize, FromRow)]
struct ReconciliationRow {
    id: String,
    account_id: String,
    account_name: String,
    reconciled_at: String,
    statement_balance: String,
    ledger_balance: String,
    difference: String,
    note: String,
    created_at: String,
}

#[derive(Debug, Serialize, FromRow)]
struct PriceAuditRow {
    instrument_id: String,
    instrument_symbol: String,
    instrument_name: String,
    price_at: String,
    price: String,
    currency: String,
    source: String,
    is_manual_override: bool,
    created_at: String,
}

#[derive(Debug, Serialize, FromRow)]
struct FxAuditRow {
    base_currency: String,
    quote_currency: String,
    rate_at: String,
    rate: String,
    source: String,
    created_at: String,
}

#[derive(Debug, Serialize, FromRow)]
struct PolicyAuditRow {
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

#[derive(Debug, Serialize, FromRow)]
struct TargetAuditRow {
    id: String,
    dimension: String,
    value: String,
    target_weight: String,
    min_weight: String,
    max_weight: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize, FromRow)]
struct DecisionAuditRow {
    id: String,
    instrument_id: Option<String>,
    instrument_name: Option<String>,
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

#[derive(Debug, Serialize, FromRow)]
struct ReviewAuditRow {
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

#[derive(Debug, Serialize, FromRow)]
struct ClassificationAuditRow {
    id: String,
    instrument_id: String,
    instrument_name: String,
    dimension: String,
    value: String,
    valid_from: String,
    created_at: String,
}

#[derive(Debug, Serialize, FromRow)]
struct ThesisAuditRow {
    id: String,
    instrument_id: String,
    instrument_name: String,
    thesis: String,
    risks: String,
    invalidation: String,
    review_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize, FromRow)]
struct DataSourceAuditRow {
    id: String,
    name: String,
    source_type: String,
    priority: i64,
    config_json: String,
    is_enabled: bool,
    has_credentials: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize, FromRow)]
struct SyncJobAuditRow {
    id: String,
    data_source_id: String,
    name: String,
    data_type: String,
    interval_seconds: i64,
    timezone: String,
    next_run_at: Option<String>,
    last_run_at: Option<String>,
    is_enabled: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize, FromRow)]
struct SyncRunAuditRow {
    id: String,
    job_id: String,
    job_name: String,
    started_at: String,
    finished_at: Option<String>,
    status: String,
    stats_json: String,
    error_message: Option<String>,
}

#[derive(Debug, Serialize)]
struct AuditData {
    accounts: Vec<Account>,
    instruments: Vec<Instrument>,
    blockchain_networks: Vec<BlockchainNetwork>,
    transaction_legs: Vec<AuditTransactionRow>,
    prices: Vec<PriceAuditRow>,
    fx_rates: Vec<FxAuditRow>,
    import_batches: Vec<ImportBatchRow>,
    reconciliations: Vec<ReconciliationRow>,
    audit_logs: Vec<AuditLogRow>,
    policy: PolicyAuditRow,
    targets: Vec<TargetAuditRow>,
    decisions: Vec<DecisionAuditRow>,
    reviews: Vec<ReviewAuditRow>,
    classifications: Vec<ClassificationAuditRow>,
    investment_theses: Vec<ThesisAuditRow>,
    data_sources: Vec<DataSourceAuditRow>,
    sync_jobs: Vec<SyncJobAuditRow>,
    sync_runs: Vec<SyncRunAuditRow>,
}

#[derive(Debug, Serialize)]
struct AuditPackage {
    manifest: AuditManifest,
    control_totals: AuditCounts,
    integrity: AuditIntegrity,
    data: AuditData,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/audit-export/summary", get(export_summary))
        .route("/audit-export/package", get(export_package))
        .route(
            "/audit-export/transactions.csv",
            get(export_transactions_csv),
        )
        .route("/audit-export/changes.csv", get(export_changes_csv))
        .route(
            "/audit-export/reconciliations.csv",
            get(export_reconciliations_csv),
        )
}

async fn export_summary(
    State(state): State<AppState>,
    Query(query): Query<AuditQuery>,
) -> AppResult<Json<AuditSummary>> {
    let period = validate_period(&query)?;
    Ok(Json(build_summary(&state, &period).await?))
}

async fn export_package(
    State(state): State<AppState>,
    Query(query): Query<AuditQuery>,
) -> AppResult<Response<Body>> {
    let period = validate_period(&query)?;
    let include_market = query.include_market.unwrap_or(true);
    let summary = build_summary(&state, &period).await?;
    let data = load_audit_data(&state.db, &period, include_market).await?;
    let serialized_data = serde_json::to_vec(&data)
        .map_err(|_| AppError::External("生成审计数据摘要失败".to_owned()))?;
    let data_sha256 = format!("{:x}", Sha256::digest(&serialized_data));
    let package = AuditPackage {
        manifest: AuditManifest {
            export_id: Uuid::now_v7().to_string(),
            generated_at: summary.generated_at.clone(),
            schema_version: AUDIT_SCHEMA_VERSION,
            period: summary.period.clone(),
            report_currency: summary.report_currency.clone(),
            timezone: summary.timezone.clone(),
            includes_market_history: include_market,
            data_sha256,
            note: "data_sha256 用于核对本审计包 data 节点是否发生变化；导出不包含密码、会话或 API 密钥。",
        },
        control_totals: summary.counts,
        integrity: summary.integrity,
        data,
    };
    let bytes = serde_json::to_vec_pretty(&package)
        .map_err(|_| AppError::External("生成审计包失败".to_owned()))?;
    downloadable(
        bytes,
        "application/json; charset=utf-8",
        format!("sanyu-audit-package-{}.json", file_period(&period)),
    )
}

async fn export_transactions_csv(
    State(state): State<AppState>,
    Query(query): Query<AuditQuery>,
) -> AppResult<Response<Body>> {
    let period = validate_period(&query)?;
    let rows = load_transactions(&state.db, &period).await?;
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(Vec::new());
    writer
        .write_record([
            "流水ID",
            "流水状态",
            "事件类型",
            "交易时间",
            "结算时间",
            "写入时间",
            "数据来源",
            "外部流水号",
            "流水备注",
            "冲销原流水ID",
            "分录ID",
            "分录序号",
            "账户ID",
            "账户名称",
            "机构",
            "标的ID",
            "标的代码",
            "标的名称",
            "资产类别",
            "分录类型",
            "数量",
            "单价",
            "计价币种",
            "分录备注",
        ])
        .map_err(csv_error)?;
    for row in rows {
        writer.serialize(row).map_err(csv_error)?;
    }
    downloadable(
        csv_with_bom(writer)?,
        "text/csv; charset=utf-8",
        format!("sanyu-audit-transactions-{}.csv", file_period(&period)),
    )
}

async fn export_changes_csv(
    State(state): State<AppState>,
    Query(query): Query<AuditQuery>,
) -> AppResult<Response<Body>> {
    let period = validate_period(&query)?;
    let rows = load_audit_logs(&state.db, &period).await?;
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(Vec::new());
    writer
        .write_record([
            "审计记录ID",
            "实体类型",
            "实体ID",
            "动作",
            "变更前JSON",
            "变更后JSON",
            "记录时间",
        ])
        .map_err(csv_error)?;
    for row in rows {
        writer.serialize(row).map_err(csv_error)?;
    }
    downloadable(
        csv_with_bom(writer)?,
        "text/csv; charset=utf-8",
        format!("sanyu-audit-changes-{}.csv", file_period(&period)),
    )
}

async fn export_reconciliations_csv(
    State(state): State<AppState>,
    Query(query): Query<AuditQuery>,
) -> AppResult<Response<Body>> {
    let period = validate_period(&query)?;
    let rows = load_reconciliations(&state.db, &period).await?;
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(Vec::new());
    writer
        .write_record([
            "对账记录ID",
            "账户ID",
            "账户名称",
            "对账时间",
            "对账单余额",
            "账本余额",
            "差异",
            "备注",
            "写入时间",
        ])
        .map_err(csv_error)?;
    for row in rows {
        writer.serialize(row).map_err(csv_error)?;
    }
    downloadable(
        csv_with_bom(writer)?,
        "text/csv; charset=utf-8",
        format!("sanyu-audit-reconciliations-{}.csv", file_period(&period)),
    )
}

fn validate_period(query: &AuditQuery) -> AppResult<ValidatedPeriod> {
    let from = query
        .from
        .as_deref()
        .map(|value| validate_date(value, "开始日期"))
        .transpose()?;
    let to = query
        .to
        .as_deref()
        .map(|value| validate_date(value, "结束日期"))
        .transpose()?;
    if let (Some(from), Some(to)) = (&from, &to)
        && from > to
    {
        return Err(AppError::Validation("开始日期不能晚于结束日期".to_owned()));
    }
    Ok(ValidatedPeriod { from, to })
}

fn validate_date(value: &str, label: &str) -> AppResult<String> {
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
        .map(|date| date.format("%Y-%m-%d").to_string())
        .map_err(|_| AppError::Validation(format!("{label}必须使用 YYYY-MM-DD 格式")))
}

async fn build_summary(state: &AppState, period: &ValidatedPeriod) -> AppResult<AuditSummary> {
    let settings = settings::load(state).await?;
    let accounts = scalar(&state.db, "SELECT COUNT(*) FROM accounts").await?;
    let active_accounts =
        scalar(&state.db, "SELECT COUNT(*) FROM accounts WHERE is_active=1").await?;
    let instruments = scalar(&state.db, "SELECT COUNT(*) FROM instruments").await?;
    let active_instruments = scalar(
        &state.db,
        "SELECT COUNT(*) FROM instruments WHERE is_active=1",
    )
    .await?;
    let transactions = period_count(&state.db, "SELECT COUNT(*) FROM transactions t WHERE (? IS NULL OR date(t.trade_at)>=date(?)) AND (? IS NULL OR date(t.trade_at)<=date(?))", period).await?;
    let transaction_legs = period_count(&state.db, "SELECT COUNT(*) FROM transaction_legs l JOIN transactions t ON t.id=l.transaction_id WHERE (? IS NULL OR date(t.trade_at)>=date(?)) AND (? IS NULL OR date(t.trade_at)<=date(?))", period).await?;
    let reversed_transactions = period_count(&state.db, "SELECT COUNT(*) FROM transactions t WHERE t.status='reversed' AND (? IS NULL OR date(t.trade_at)>=date(?)) AND (? IS NULL OR date(t.trade_at)<=date(?))", period).await?;
    let prices = period_count(&state.db, "SELECT COUNT(*) FROM prices p WHERE (? IS NULL OR date(p.price_at)>=date(?)) AND (? IS NULL OR date(p.price_at)<=date(?))", period).await?;
    let fx_rates = period_count(&state.db, "SELECT COUNT(*) FROM fx_rates f WHERE (? IS NULL OR date(f.rate_at)>=date(?)) AND (? IS NULL OR date(f.rate_at)<=date(?))", period).await?;
    let import_batches = period_count(&state.db, "SELECT COUNT(*) FROM import_batches b WHERE (? IS NULL OR date(b.imported_at)>=date(?)) AND (? IS NULL OR date(b.imported_at)<=date(?))", period).await?;
    let reconciliations = period_count(&state.db, "SELECT COUNT(*) FROM reconciliations r WHERE (? IS NULL OR date(r.reconciled_at)>=date(?)) AND (? IS NULL OR date(r.reconciled_at)<=date(?))", period).await?;
    let audit_logs = period_count(&state.db, "SELECT COUNT(*) FROM audit_logs a WHERE (? IS NULL OR date(a.created_at)>=date(?)) AND (? IS NULL OR date(a.created_at)<=date(?))", period).await?;
    let decisions = period_count(&state.db, "SELECT COUNT(*) FROM decision_logs d WHERE (? IS NULL OR date(d.decided_at)>=date(?)) AND (? IS NULL OR date(d.decided_at)<=date(?))", period).await?;
    let reviews = period_count(&state.db, "SELECT COUNT(*) FROM reviews r WHERE (? IS NULL OR date(r.period_end)>=date(?)) AND (? IS NULL OR date(r.period_start)<=date(?))", period).await?;
    let sync_runs = period_count(&state.db, "SELECT COUNT(*) FROM sync_runs s WHERE (? IS NULL OR date(s.started_at)>=date(?)) AND (? IS NULL OR date(s.started_at)<=date(?))", period).await?;

    let orphan_legs = scalar(
        &state.db,
        "SELECT COUNT(*) FROM transaction_legs l LEFT JOIN transactions t ON t.id=l.transaction_id WHERE t.id IS NULL",
    )
    .await?;
    let transactions_without_legs = period_count(&state.db, "SELECT COUNT(*) FROM transactions t LEFT JOIN transaction_legs l ON l.transaction_id=t.id WHERE l.id IS NULL AND (? IS NULL OR date(t.trade_at)>=date(?)) AND (? IS NULL OR date(t.trade_at)<=date(?))", period).await?;
    let duplicate_external_ids = period_count(&state.db, "SELECT COUNT(*) FROM (SELECT t.source,t.external_id FROM transactions t WHERE t.external_id IS NOT NULL AND (? IS NULL OR date(t.trade_at)>=date(?)) AND (? IS NULL OR date(t.trade_at)<=date(?)) GROUP BY t.source,t.external_id HAVING COUNT(*)>1)", period).await?;
    let missing_audit_trail = period_count(&state.db, "SELECT COUNT(*) FROM transactions t LEFT JOIN audit_logs a ON a.entity_type='transaction' AND a.entity_id=t.id WHERE a.id IS NULL AND (? IS NULL OR date(t.trade_at)>=date(?)) AND (? IS NULL OR date(t.trade_at)<=date(?))", period).await?;
    let reconciliation_differences = period_count(&state.db, "SELECT COUNT(*) FROM reconciliations r WHERE CAST(r.difference AS REAL)<>0 AND (? IS NULL OR date(r.reconciled_at)>=date(?)) AND (? IS NULL OR date(r.reconciled_at)<=date(?))", period).await?;
    let accounts_without_reconciliation = scalar(&state.db, "SELECT COUNT(*) FROM accounts a WHERE a.is_active=1 AND NOT EXISTS(SELECT 1 FROM reconciliations r WHERE r.account_id=a.id)").await?;

    let critical_issue_count =
        orphan_legs + transactions_without_legs + duplicate_external_ids + missing_audit_trail;
    let warning_count = reconciliation_differences + accounts_without_reconciliation;
    let checks = vec![
        check(
            "orphan_legs",
            "孤立分录",
            "critical",
            orphan_legs,
            "分录必须关联有效流水",
        ),
        check(
            "transactions_without_legs",
            "空流水",
            "critical",
            transactions_without_legs,
            "每条流水至少应包含一条分录",
        ),
        check(
            "duplicate_external_ids",
            "重复外部流水号",
            "critical",
            duplicate_external_ids,
            "同一来源与外部流水号不应重复",
        ),
        check(
            "missing_audit_trail",
            "缺失审计轨迹",
            "critical",
            missing_audit_trail,
            "流水创建或冲销应存在审计记录",
        ),
        check(
            "reconciliation_differences",
            "未消除对账差异",
            "warning",
            reconciliation_differences,
            "差异不为零的对账记录需要复核",
        ),
        check(
            "accounts_without_reconciliation",
            "尚未对账账户",
            "warning",
            accounts_without_reconciliation,
            "启用账户尚无对账记录",
        ),
    ];
    Ok(AuditSummary {
        generated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        schema_version: AUDIT_SCHEMA_VERSION,
        period: AuditPeriod {
            from: period.from.clone(),
            to: period.to.clone(),
        },
        report_currency: settings.report_currency,
        timezone: settings.timezone,
        counts: AuditCounts {
            accounts,
            active_accounts,
            instruments,
            active_instruments,
            transactions,
            transaction_legs,
            reversed_transactions,
            prices,
            fx_rates,
            import_batches,
            reconciliations,
            audit_logs,
            decisions,
            reviews,
            sync_runs,
        },
        integrity: AuditIntegrity {
            passed: critical_issue_count == 0,
            critical_issue_count,
            warning_count,
            checks,
        },
    })
}

fn check(
    code: &'static str,
    label: &'static str,
    level: &'static str,
    count: i64,
    detail: &str,
) -> AuditCheck {
    AuditCheck {
        code,
        label,
        level,
        count,
        detail: detail.to_owned(),
    }
}

async fn load_audit_data(
    pool: &SqlitePool,
    period: &ValidatedPeriod,
    include_market: bool,
) -> AppResult<AuditData> {
    let accounts = sqlx::query_as::<_, Account>("SELECT * FROM accounts ORDER BY name,id")
        .fetch_all(pool)
        .await?;
    let instruments =
        sqlx::query_as::<_, Instrument>("SELECT * FROM instruments ORDER BY asset_type,name,id")
            .fetch_all(pool)
            .await?;
    let blockchain_networks = sqlx::query_as::<_, BlockchainNetwork>(
        "SELECT * FROM blockchain_networks ORDER BY name,id",
    )
    .fetch_all(pool)
    .await?;
    let transaction_legs = load_transactions(pool, period).await?;
    let prices = if include_market {
        load_prices(pool, period).await?
    } else {
        Vec::new()
    };
    let fx_rates = if include_market {
        load_fx_rates(pool, period).await?
    } else {
        Vec::new()
    };
    let import_batches = load_import_batches(pool, period).await?;
    let reconciliations = load_reconciliations(pool, period).await?;
    let audit_logs = load_audit_logs(pool, period).await?;
    let policy = sqlx::query_as::<_, PolicyAuditRow>("SELECT objective,horizon_years,max_drawdown,max_single_position,max_high_risk,emergency_fund_months,allowed_tools,prohibited_tools,rebalance_frequency,review_frequency,updated_at FROM investment_policy WHERE id=1").fetch_one(pool).await?;
    let targets =
        sqlx::query_as::<_, TargetAuditRow>("SELECT * FROM targets ORDER BY dimension,value,id")
            .fetch_all(pool)
            .await?;
    let decisions = period_rows::<DecisionAuditRow>(pool, "SELECT d.id,d.instrument_id,i.name AS instrument_name,d.action,d.decided_at,d.rationale,d.confidence,d.risks,d.invalidation,d.review_at,d.outcome,d.process_score,d.result_score,d.created_at,d.updated_at FROM decision_logs d LEFT JOIN instruments i ON i.id=d.instrument_id WHERE (? IS NULL OR date(d.decided_at)>=date(?)) AND (? IS NULL OR date(d.decided_at)<=date(?)) ORDER BY d.decided_at,d.id", period).await?;
    let reviews = period_rows::<ReviewAuditRow>(pool, "SELECT * FROM reviews r WHERE (? IS NULL OR date(r.period_end)>=date(?)) AND (? IS NULL OR date(r.period_start)<=date(?)) ORDER BY r.period_start,r.id", period).await?;
    let classifications = sqlx::query_as::<_, ClassificationAuditRow>("SELECT c.id,c.instrument_id,i.name AS instrument_name,c.dimension,c.value,c.valid_from,c.created_at FROM classifications c JOIN instruments i ON i.id=c.instrument_id ORDER BY c.valid_from,c.id").fetch_all(pool).await?;
    let investment_theses = sqlx::query_as::<_, ThesisAuditRow>("SELECT t.id,t.instrument_id,i.name AS instrument_name,t.thesis,t.risks,t.invalidation,t.review_at,t.created_at,t.updated_at FROM investment_theses t JOIN instruments i ON i.id=t.instrument_id ORDER BY t.created_at,t.id").fetch_all(pool).await?;
    let data_sources = sqlx::query_as::<_, DataSourceAuditRow>("SELECT id,name,source_type,priority,config_json,is_enabled,credentials_ref IS NOT NULL AS has_credentials,created_at,updated_at FROM data_sources ORDER BY priority,name,id").fetch_all(pool).await?;
    let sync_jobs = sqlx::query_as::<_, SyncJobAuditRow>("SELECT id,data_source_id,name,data_type,interval_seconds,timezone,next_run_at,last_run_at,is_enabled,created_at,updated_at FROM sync_jobs ORDER BY name,id").fetch_all(pool).await?;
    let sync_runs = period_rows::<SyncRunAuditRow>(pool, "SELECT r.id,r.job_id,j.name AS job_name,r.started_at,r.finished_at,r.status,r.stats_json,r.error_message FROM sync_runs r JOIN sync_jobs j ON j.id=r.job_id WHERE (? IS NULL OR date(r.started_at)>=date(?)) AND (? IS NULL OR date(r.started_at)<=date(?)) ORDER BY r.started_at,r.id", period).await?;
    Ok(AuditData {
        accounts,
        instruments,
        blockchain_networks,
        transaction_legs,
        prices,
        fx_rates,
        import_batches,
        reconciliations,
        audit_logs,
        policy,
        targets,
        decisions,
        reviews,
        classifications,
        investment_theses,
        data_sources,
        sync_jobs,
        sync_runs,
    })
}

async fn load_transactions(
    pool: &SqlitePool,
    period: &ValidatedPeriod,
) -> AppResult<Vec<AuditTransactionRow>> {
    period_rows(pool, r#"SELECT t.id AS transaction_id,t.status AS transaction_status,t.transaction_type,t.trade_at,t.settle_at,t.created_at AS transaction_created_at,t.source,t.external_id,t.memo AS transaction_memo,t.reverses_transaction_id,l.id AS leg_id,l.sequence AS leg_sequence,l.account_id,a.name AS account_name,a.institution,l.instrument_id,i.symbol AS instrument_symbol,i.name AS instrument_name,i.asset_type,l.leg_type,l.quantity,l.unit_price,l.price_currency,l.memo AS leg_memo FROM transactions t JOIN transaction_legs l ON l.transaction_id=t.id JOIN accounts a ON a.id=l.account_id JOIN instruments i ON i.id=l.instrument_id WHERE (? IS NULL OR date(t.trade_at)>=date(?)) AND (? IS NULL OR date(t.trade_at)<=date(?)) ORDER BY t.trade_at,t.id,l.sequence"#, period).await
}

async fn load_audit_logs(
    pool: &SqlitePool,
    period: &ValidatedPeriod,
) -> AppResult<Vec<AuditLogRow>> {
    period_rows(pool, "SELECT * FROM audit_logs a WHERE (? IS NULL OR date(a.created_at)>=date(?)) AND (? IS NULL OR date(a.created_at)<=date(?)) ORDER BY a.created_at,a.id", period).await
}

async fn load_import_batches(
    pool: &SqlitePool,
    period: &ValidatedPeriod,
) -> AppResult<Vec<ImportBatchRow>> {
    period_rows(pool, "SELECT * FROM import_batches b WHERE (? IS NULL OR date(b.imported_at)>=date(?)) AND (? IS NULL OR date(b.imported_at)<=date(?)) ORDER BY b.imported_at,b.id", period).await
}

async fn load_reconciliations(
    pool: &SqlitePool,
    period: &ValidatedPeriod,
) -> AppResult<Vec<ReconciliationRow>> {
    period_rows(pool, "SELECT r.id,r.account_id,a.name AS account_name,r.reconciled_at,r.statement_balance,r.ledger_balance,r.difference,r.note,r.created_at FROM reconciliations r JOIN accounts a ON a.id=r.account_id WHERE (? IS NULL OR date(r.reconciled_at)>=date(?)) AND (? IS NULL OR date(r.reconciled_at)<=date(?)) ORDER BY r.reconciled_at,r.id", period).await
}

async fn load_prices(pool: &SqlitePool, period: &ValidatedPeriod) -> AppResult<Vec<PriceAuditRow>> {
    period_rows(pool, "SELECT p.instrument_id,i.symbol AS instrument_symbol,i.name AS instrument_name,p.price_at,p.price,p.currency,p.source,p.is_manual_override,p.created_at FROM prices p JOIN instruments i ON i.id=p.instrument_id WHERE (? IS NULL OR date(p.price_at)>=date(?)) AND (? IS NULL OR date(p.price_at)<=date(?)) ORDER BY p.price_at,p.instrument_id,p.source", period).await
}

async fn load_fx_rates(pool: &SqlitePool, period: &ValidatedPeriod) -> AppResult<Vec<FxAuditRow>> {
    period_rows(pool, "SELECT * FROM fx_rates f WHERE (? IS NULL OR date(f.rate_at)>=date(?)) AND (? IS NULL OR date(f.rate_at)<=date(?)) ORDER BY f.rate_at,f.base_currency,f.quote_currency,f.source", period).await
}

async fn scalar(pool: &SqlitePool, sql: &str) -> AppResult<i64> {
    Ok(sqlx::query_scalar(sql).fetch_one(pool).await?)
}

async fn period_count(pool: &SqlitePool, sql: &str, period: &ValidatedPeriod) -> AppResult<i64> {
    Ok(sqlx::query_scalar(sql)
        .bind(period.from.as_deref())
        .bind(period.from.as_deref())
        .bind(period.to.as_deref())
        .bind(period.to.as_deref())
        .fetch_one(pool)
        .await?)
}

async fn period_rows<T>(pool: &SqlitePool, sql: &str, period: &ValidatedPeriod) -> AppResult<Vec<T>>
where
    for<'row> T: FromRow<'row, sqlx::sqlite::SqliteRow> + Send + Unpin,
{
    Ok(sqlx::query_as::<_, T>(sql)
        .bind(period.from.as_deref())
        .bind(period.from.as_deref())
        .bind(period.to.as_deref())
        .bind(period.to.as_deref())
        .fetch_all(pool)
        .await?)
}

fn csv_with_bom(writer: csv::Writer<Vec<u8>>) -> AppResult<Vec<u8>> {
    let contents = writer
        .into_inner()
        .map_err(|_| AppError::External("生成审计 CSV 失败".to_owned()))?;
    let mut bytes = b"\xEF\xBB\xBF".to_vec();
    bytes.extend(contents);
    Ok(bytes)
}

fn csv_error(_: csv::Error) -> AppError {
    AppError::External("生成审计 CSV 失败".to_owned())
}

fn downloadable(
    bytes: Vec<u8>,
    content_type: &'static str,
    filename: String,
) -> AppResult<Response<Body>> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .header("x-content-type-options", "nosniff")
        .body(Body::from(bytes))
        .map_err(|_| AppError::External("生成下载响应失败".to_owned()))
}

fn file_period(period: &ValidatedPeriod) -> String {
    match (&period.from, &period.to) {
        (None, None) => "all-history".to_owned(),
        (Some(from), None) => format!("from-{from}"),
        (None, Some(to)) => format!("through-{to}"),
        (Some(from), Some(to)) => format!("{from}-to-{to}"),
    }
}
