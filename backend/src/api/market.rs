use super::{AppState, common};
use crate::error::{AppError, AppResult};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get},
};
use chrono::{SecondsFormat, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;
use std::{collections::HashMap, str::FromStr};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PriceRecord {
    pub instrument_id: String,
    pub price_at: String,
    pub price: String,
    pub currency: String,
    pub source: String,
    pub is_manual_override: bool,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
struct PriceInput {
    instrument_id: String,
    price_at: Option<String>,
    price: String,
    currency: String,
    source: Option<String>,
    #[serde(default = "default_true")]
    is_manual_override: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct FxRecord {
    pub base_currency: String,
    pub quote_currency: String,
    pub rate_at: String,
    pub rate: String,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
struct FxInput {
    base_currency: String,
    quote_currency: String,
    rate_at: Option<String>,
    rate: String,
    source: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MarketQuery {
    instrument_id: Option<String>,
    limit: Option<i64>,
}

fn default_true() -> bool {
    true
}

const MAJOR_CURRENCIES: [&str; 11] = common::MAJOR_CURRENCIES;

#[derive(Debug, Serialize)]
struct MajorFxRefreshResult {
    source: &'static str,
    currencies: Vec<&'static str>,
    pairs_written: usize,
    reference_dates: Vec<String>,
    refreshed_at: String,
}

#[derive(Debug, Serialize)]
struct CryptoUsdQuote {
    instrument_id: String,
    coin_id: &'static str,
    symbol: &'static str,
    name: &'static str,
    asset_type: &'static str,
    price: String,
    currency: &'static str,
}

#[derive(Debug, Serialize)]
struct CryptoUsdRefreshResult {
    source: &'static str,
    instruments_created: usize,
    prices_written: usize,
    missing_symbols: Vec<&'static str>,
    quotes: Vec<CryptoUsdQuote>,
    refreshed_at: String,
}

struct CryptoAssetSpec {
    coin_id: &'static str,
    symbol: &'static str,
    name: &'static str,
    asset_type: &'static str,
    precision: i64,
}

const MAINSTREAM_CRYPTO_USD: [CryptoAssetSpec; 15] = [
    CryptoAssetSpec {
        coin_id: "bitcoin",
        symbol: "BTC",
        name: "Bitcoin",
        asset_type: "crypto",
        precision: 8,
    },
    CryptoAssetSpec {
        coin_id: "ethereum",
        symbol: "ETH",
        name: "Ethereum",
        asset_type: "crypto",
        precision: 8,
    },
    CryptoAssetSpec {
        coin_id: "binancecoin",
        symbol: "BNB",
        name: "BNB",
        asset_type: "crypto",
        precision: 8,
    },
    CryptoAssetSpec {
        coin_id: "solana",
        symbol: "SOL",
        name: "Solana",
        asset_type: "crypto",
        precision: 9,
    },
    CryptoAssetSpec {
        coin_id: "ripple",
        symbol: "XRP",
        name: "XRP",
        asset_type: "crypto",
        precision: 6,
    },
    CryptoAssetSpec {
        coin_id: "cardano",
        symbol: "ADA",
        name: "Cardano",
        asset_type: "crypto",
        precision: 6,
    },
    CryptoAssetSpec {
        coin_id: "dogecoin",
        symbol: "DOGE",
        name: "Dogecoin",
        asset_type: "crypto",
        precision: 8,
    },
    CryptoAssetSpec {
        coin_id: "tron",
        symbol: "TRX",
        name: "TRON",
        asset_type: "crypto",
        precision: 6,
    },
    CryptoAssetSpec {
        coin_id: "avalanche-2",
        symbol: "AVAX",
        name: "Avalanche",
        asset_type: "crypto",
        precision: 9,
    },
    CryptoAssetSpec {
        coin_id: "polkadot",
        symbol: "DOT",
        name: "Polkadot",
        asset_type: "crypto",
        precision: 10,
    },
    CryptoAssetSpec {
        coin_id: "tether",
        symbol: "USDT",
        name: "Tether",
        asset_type: "stablecoin",
        precision: 6,
    },
    CryptoAssetSpec {
        coin_id: "usd-coin",
        symbol: "USDC",
        name: "USDC",
        asset_type: "stablecoin",
        precision: 6,
    },
    CryptoAssetSpec {
        coin_id: "dai",
        symbol: "DAI",
        name: "Dai",
        asset_type: "stablecoin",
        precision: 18,
    },
    CryptoAssetSpec {
        coin_id: "first-digital-usd",
        symbol: "FDUSD",
        name: "First Digital USD",
        asset_type: "stablecoin",
        precision: 6,
    },
    CryptoAssetSpec {
        coin_id: "paypal-usd",
        symbol: "PYUSD",
        name: "PayPal USD",
        asset_type: "stablecoin",
        precision: 6,
    },
];

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/prices", get(list_prices).post(upsert_price))
        .route(
            "/prices/refresh-crypto-usd",
            axum::routing::post(refresh_crypto_usd),
        )
        .route("/prices/{instrument_id}", delete(delete_prices))
        .route("/fx-rates", get(list_fx).post(upsert_fx))
        .route(
            "/fx-rates/refresh-major",
            axum::routing::post(refresh_major_fx),
        )
}

async fn refresh_crypto_usd(
    State(state): State<AppState>,
) -> AppResult<Json<CryptoUsdRefreshResult>> {
    let coin_ids = MAINSTREAM_CRYPTO_USD
        .iter()
        .map(|asset| asset.coin_id)
        .collect::<Vec<_>>()
        .join(",");
    let url =
        format!("https://api.coingecko.com/api/v3/simple/price?ids={coin_ids}&vs_currencies=usd");
    let response = super::settings::http_client(&state.db, std::time::Duration::from_secs(20))
        .await?
        .get(url)
        .header("user-agent", "SANYU-Invest/0.1")
        .send()
        .await
        .map_err(|error| AppError::External(format!("虚拟货币美元价格请求失败：{error}")))?;
    if !response.status().is_success() {
        return Err(AppError::External(format!(
            "CoinGecko 返回状态 {}",
            response.status()
        )));
    }
    let body = response
        .json::<Value>()
        .await
        .map_err(|error| AppError::External(format!("虚拟货币价格响应格式错误：{error}")))?;
    let refreshed_at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let mut transaction = state.db.begin().await?;
    let mut instruments_created = 0;
    let mut quotes = Vec::new();
    let mut missing_symbols = Vec::new();

    for asset in &MAINSTREAM_CRYPTO_USD {
        let Some(raw_price) = body.get(asset.coin_id).and_then(|coin| coin.get("usd")) else {
            missing_symbols.push(asset.symbol);
            continue;
        };
        let price = common::positive_decimal(&raw_price.to_string(), "美元价格")?;
        let instrument_id = if let Some(id) = sqlx::query_scalar::<_, String>(
            r#"SELECT id FROM instruments
               WHERE upper(symbol) = ? AND asset_type IN ('crypto', 'stablecoin')
               ORDER BY is_active DESC, created_at LIMIT 1"#,
        )
        .bind(asset.symbol)
        .fetch_optional(&mut *transaction)
        .await?
        {
            id
        } else {
            let id = uuid::Uuid::now_v7().to_string();
            sqlx::query(
                r#"INSERT INTO instruments
                   (id, symbol, name, asset_type, currency, exchange, precision)
                   VALUES (?, ?, ?, ?, 'USD', 'CoinGecko', ?)"#,
            )
            .bind(&id)
            .bind(asset.symbol)
            .bind(asset.name)
            .bind(asset.asset_type)
            .bind(asset.precision)
            .execute(&mut *transaction)
            .await?;
            instruments_created += 1;
            id
        };

        sqlx::query(
            r#"INSERT INTO prices
               (instrument_id, price_at, price, currency, source, is_manual_override)
               VALUES (?, ?, ?, 'USD', 'coingecko_mainstream_usd', 0)
               ON CONFLICT(instrument_id, price_at, source) DO UPDATE SET
                 price = excluded.price, currency = excluded.currency,
                 is_manual_override = excluded.is_manual_override"#,
        )
        .bind(&instrument_id)
        .bind(&refreshed_at)
        .bind(&price)
        .execute(&mut *transaction)
        .await?;
        quotes.push(CryptoUsdQuote {
            instrument_id,
            coin_id: asset.coin_id,
            symbol: asset.symbol,
            name: asset.name,
            asset_type: asset.asset_type,
            price,
            currency: "USD",
        });
    }
    if quotes.is_empty() {
        return Err(AppError::External(
            "CoinGecko 响应中没有可用的主流币种美元价格".to_owned(),
        ));
    }
    transaction.commit().await?;
    Ok(Json(CryptoUsdRefreshResult {
        source: "CoinGecko",
        instruments_created,
        prices_written: quotes.len(),
        missing_symbols,
        quotes,
        refreshed_at,
    }))
}

pub fn spawn_major_fx_bootstrap(pool: SqlitePool) {
    tokio::spawn(async move {
        let existing: Result<i64, _> =
            sqlx::query_scalar("SELECT COUNT(*) FROM fx_rates WHERE source = 'frankfurter_major'")
                .fetch_one(&pool)
                .await;
        if matches!(existing, Ok(0))
            && let Err(error) = refresh_major_rates(&pool).await
        {
            tracing::warn!(?error, "initial major currency refresh failed");
        }
    });
}

async fn list_prices(
    State(state): State<AppState>,
    Query(query): Query<MarketQuery>,
) -> AppResult<Json<Vec<PriceRecord>>> {
    let limit = query.limit.unwrap_or(500).clamp(1, 5_000);
    let prices = if let Some(instrument_id) = query.instrument_id {
        let instrument_id = common::id(&instrument_id, "标的 ID")?;
        sqlx::query_as::<_, PriceRecord>(
            "SELECT * FROM prices WHERE instrument_id = ? ORDER BY price_at DESC LIMIT ?",
        )
        .bind(instrument_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as::<_, PriceRecord>(
            r#"SELECT p.* FROM prices p
               WHERE p.rowid = (SELECT p2.rowid FROM prices p2
                 WHERE p2.instrument_id = p.instrument_id
                 ORDER BY p2.price_at DESC, p2.is_manual_override DESC, p2.created_at DESC LIMIT 1)
               ORDER BY p.instrument_id"#,
        )
        .fetch_all(&state.db)
        .await?
    };
    Ok(Json(prices))
}

async fn upsert_price(
    State(state): State<AppState>,
    Json(input): Json<PriceInput>,
) -> AppResult<(StatusCode, Json<PriceRecord>)> {
    let instrument_id = common::id(&input.instrument_id, "标的 ID")?;
    ensure_instrument(&state, &instrument_id).await?;
    let price_at = normalize_time(input.price_at.as_deref(), "价格时间")?;
    let price = common::positive_decimal(&input.price, "价格")?;
    let currency = common::currency(&input.currency, "价格币种")?;
    let source =
        common::required_text(input.source.as_deref().unwrap_or("manual"), "价格来源", 100)?;
    sqlx::query(
        r#"INSERT INTO prices (instrument_id, price_at, price, currency, source, is_manual_override)
           VALUES (?, ?, ?, ?, ?, ?)
           ON CONFLICT(instrument_id, price_at, source) DO UPDATE SET
             price = excluded.price, currency = excluded.currency,
             is_manual_override = excluded.is_manual_override"#,
    )
    .bind(&instrument_id)
    .bind(&price_at)
    .bind(price)
    .bind(currency)
    .bind(&source)
    .bind(input.is_manual_override)
    .execute(&state.db)
    .await?;
    let record = sqlx::query_as::<_, PriceRecord>(
        "SELECT * FROM prices WHERE instrument_id = ? AND price_at = ? AND source = ?",
    )
    .bind(instrument_id)
    .bind(price_at)
    .bind(source)
    .fetch_one(&state.db)
    .await?;
    Ok((StatusCode::CREATED, Json(record)))
}

async fn delete_prices(
    State(state): State<AppState>,
    Path(instrument_id): Path<String>,
) -> AppResult<StatusCode> {
    let instrument_id = common::id(&instrument_id, "标的 ID")?;
    sqlx::query("DELETE FROM prices WHERE instrument_id = ?")
        .bind(instrument_id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_fx(State(state): State<AppState>) -> AppResult<Json<Vec<FxRecord>>> {
    let rates = sqlx::query_as::<_, FxRecord>(
        r#"SELECT f.* FROM fx_rates f
           WHERE f.rowid = (SELECT f2.rowid FROM fx_rates f2
             WHERE f2.base_currency = f.base_currency AND f2.quote_currency = f.quote_currency
             ORDER BY f2.rate_at DESC, f2.created_at DESC LIMIT 1)
           ORDER BY f.base_currency, f.quote_currency"#,
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rates))
}

async fn refresh_major_fx(State(state): State<AppState>) -> AppResult<Json<MajorFxRefreshResult>> {
    Ok(Json(refresh_major_rates(&state.db).await?))
}

async fn refresh_major_rates(pool: &SqlitePool) -> AppResult<MajorFxRefreshResult> {
    let quotes = MAJOR_CURRENCIES
        .iter()
        .copied()
        .filter(|currency| *currency != "EUR")
        .collect::<Vec<_>>()
        .join(",");
    let url =
        format!("https://api.frankfurter.dev/v2/rates?base=EUR&quotes={quotes}&providers=ECB");
    let response = super::settings::http_client(pool, std::time::Duration::from_secs(20))
        .await?
        .get(url)
        .header("user-agent", "SANYU-Invest/0.1")
        .send()
        .await
        .map_err(|error| AppError::External(format!("主流汇率请求失败：{error}")))?;
    if !response.status().is_success() {
        return Err(AppError::External(format!(
            "主流汇率服务返回状态 {}",
            response.status()
        )));
    }
    let rows = response
        .json::<Vec<Value>>()
        .await
        .map_err(|error| AppError::External(format!("主流汇率响应格式错误：{error}")))?;
    let mut eur_rates = HashMap::from([("EUR".to_owned(), Decimal::ONE)]);
    let mut dates = Vec::new();
    for row in rows {
        let quote = row["quote"]
            .as_str()
            .ok_or_else(|| AppError::External("汇率响应缺少 quote".to_owned()))?;
        let rate = Decimal::from_str(&row["rate"].to_string())
            .map_err(|_| AppError::External("汇率响应包含无效数值".to_owned()))?;
        eur_rates.insert(quote.to_owned(), rate);
        if let Some(date) = row["date"].as_str()
            && !dates.iter().any(|item| item == date)
        {
            dates.push(date.to_owned());
        }
    }
    let missing = MAJOR_CURRENCIES
        .iter()
        .filter(|currency| !eur_rates.contains_key(**currency))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(AppError::External(format!(
            "汇率源缺少币种：{}",
            missing.join(", ")
        )));
    }
    if dates.len() != 1 {
        return Err(AppError::External(format!(
            "主流汇率必须来自同一参考日，实际为：{}",
            dates.join(", ")
        )));
    }
    let rate_at = format!("{}T00:00:00.000Z", dates[0]);
    let refreshed_at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let mut transaction = pool.begin().await?;
    let mut pairs_written = 0;
    for base in MAJOR_CURRENCIES {
        for quote in MAJOR_CURRENCIES {
            if base == quote {
                continue;
            }
            let rate = eur_rates[quote] / eur_rates[base];
            sqlx::query(
                r#"INSERT INTO fx_rates(base_currency,quote_currency,rate_at,rate,source)
                   VALUES(?,?,?,?,'frankfurter_major')
                   ON CONFLICT(base_currency,quote_currency,rate_at,source)
                   DO UPDATE SET rate=excluded.rate"#,
            )
            .bind(base)
            .bind(quote)
            .bind(&rate_at)
            .bind(rate.round_dp(12).normalize().to_string())
            .execute(&mut *transaction)
            .await?;
            pairs_written += 1;
        }
    }
    transaction.commit().await?;
    Ok(MajorFxRefreshResult {
        source: "Frankfurter / ECB",
        currencies: MAJOR_CURRENCIES.to_vec(),
        pairs_written,
        reference_dates: dates,
        refreshed_at,
    })
}

async fn upsert_fx(
    State(state): State<AppState>,
    Json(input): Json<FxInput>,
) -> AppResult<(StatusCode, Json<FxRecord>)> {
    let base = common::major_currency(&input.base_currency, "基础币种")?;
    let quote = common::major_currency(&input.quote_currency, "报价币种")?;
    if base == quote {
        return Err(AppError::Validation(
            "汇率的基础币种和报价币种不能相同".to_owned(),
        ));
    }
    let rate_at = normalize_time(input.rate_at.as_deref(), "汇率时间")?;
    let rate = common::positive_decimal(&input.rate, "汇率")?;
    let source =
        common::required_text(input.source.as_deref().unwrap_or("manual"), "汇率来源", 100)?;
    sqlx::query(
        r#"INSERT INTO fx_rates (base_currency, quote_currency, rate_at, rate, source)
           VALUES (?, ?, ?, ?, ?)
           ON CONFLICT(base_currency, quote_currency, rate_at, source) DO UPDATE SET rate = excluded.rate"#,
    )
    .bind(&base)
    .bind(&quote)
    .bind(&rate_at)
    .bind(rate)
    .bind(&source)
    .execute(&state.db)
    .await?;
    let record = sqlx::query_as::<_, FxRecord>(
        r#"SELECT * FROM fx_rates
           WHERE base_currency = ? AND quote_currency = ? AND rate_at = ? AND source = ?"#,
    )
    .bind(base)
    .bind(quote)
    .bind(rate_at)
    .bind(source)
    .fetch_one(&state.db)
    .await?;
    Ok((StatusCode::CREATED, Json(record)))
}

async fn ensure_instrument(state: &AppState, id: &str) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM instruments WHERE id = ?)")
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    if !exists {
        return Err(AppError::NotFound("标的不存在".to_owned()));
    }
    Ok(())
}

fn normalize_time(value: Option<&str>, field: &str) -> AppResult<String> {
    value
        .map(|value| common::rfc3339(value, field))
        .unwrap_or_else(|| Ok(Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)))
}
