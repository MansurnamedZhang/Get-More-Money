use super::{AppState, settings};
use crate::error::AppResult;
use axum::{Json, Router, extract::State, routing::get};
use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde::Serialize;
use std::{collections::HashMap, str::FromStr};

#[derive(Debug, sqlx::FromRow)]
struct LedgerLeg {
    transaction_type: String,
    trade_at: String,
    account_id: String,
    account_name: String,
    account_type: String,
    instrument_id: String,
    symbol: String,
    instrument_name: String,
    asset_type: String,
    currency: String,
    leg_type: String,
    quantity: String,
    unit_price: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct LatestPrice {
    instrument_id: String,
    price_at: String,
    price: String,
    currency: String,
    source: String,
}

#[derive(Debug, sqlx::FromRow)]
struct LatestFx {
    base_currency: String,
    quote_currency: String,
    rate: String,
}

#[derive(Debug, Default)]
struct PositionState {
    quantity: Decimal,
    cost_basis: Decimal,
    realized: Decimal,
    last_trade_price: Option<Decimal>,
}

#[derive(Debug, Serialize)]
pub struct Holding {
    account_id: String,
    account_name: String,
    account_type: String,
    instrument_id: String,
    symbol: String,
    name: String,
    asset_type: String,
    currency: String,
    quantity: String,
    average_cost: String,
    price: String,
    price_source: String,
    price_at: Option<String>,
    market_value: String,
    cost_basis: String,
    unrealized_pnl: String,
    realized_pnl: String,
    weight: String,
    stale: bool,
    missing_price: bool,
    missing_fx: bool,
}

#[derive(Debug, Serialize)]
struct AllocationItem {
    key: String,
    value: String,
    weight: String,
}

#[derive(Debug, Serialize)]
struct RiskSummary {
    max_position_weight: String,
    crypto_weight: String,
    cash_weight: String,
    account_concentration: String,
    stale_price_count: usize,
    missing_price_count: usize,
    missing_fx_count: usize,
    target_breaches: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PerformanceSummary {
    xirr: Option<String>,
    twr: Option<String>,
    realized_pnl: String,
    unrealized_pnl: String,
    income: String,
    fees_and_taxes: String,
    note: String,
}

#[derive(Debug, Serialize)]
pub struct PortfolioSummary {
    report_currency: String,
    total_market_value: String,
    investment_value: String,
    cash_value: String,
    total_cost_basis: String,
    holdings: Vec<Holding>,
    allocation_by_asset_type: Vec<AllocationItem>,
    allocation_by_account: Vec<AllocationItem>,
    allocation_by_currency: Vec<AllocationItem>,
    risk: RiskSummary,
    performance: PerformanceSummary,
    calculated_at: String,
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/portfolio/summary", get(summary))
}

async fn summary(State(state): State<AppState>) -> AppResult<Json<PortfolioSummary>> {
    Ok(Json(calculate(&state).await?))
}

pub async fn calculate(state: &AppState) -> AppResult<PortfolioSummary> {
    let config = settings::load(state).await?;
    let legs = sqlx::query_as::<_, LedgerLeg>(
        r#"SELECT t.transaction_type,t.trade_at,l.account_id,a.name account_name,
                  a.account_type,l.instrument_id,i.symbol,i.name instrument_name,
                  i.asset_type,i.currency,l.leg_type,l.quantity,l.unit_price
           FROM transaction_legs l
           JOIN transactions t ON t.id=l.transaction_id
           JOIN accounts a ON a.id=l.account_id
           JOIN instruments i ON i.id=l.instrument_id
           WHERE t.status='confirmed' AND t.reverses_transaction_id IS NULL
             AND a.is_active=1 AND a.include_in_net_worth=1
           ORDER BY t.trade_at,t.id,l.sequence"#,
    )
    .fetch_all(&state.db)
    .await?;
    let prices = sqlx::query_as::<_, LatestPrice>(
        r#"SELECT p.instrument_id,p.price_at,p.price,p.currency,p.source FROM prices p
           WHERE p.rowid=(SELECT p2.rowid FROM prices p2 WHERE p2.instrument_id=p.instrument_id
             ORDER BY p2.price_at DESC,p2.is_manual_override DESC,p2.created_at DESC LIMIT 1)"#,
    )
    .fetch_all(&state.db)
    .await?;
    let rates = sqlx::query_as::<_, LatestFx>(
        r#"SELECT f.base_currency,f.quote_currency,f.rate FROM fx_rates f
           WHERE f.rowid=(SELECT f2.rowid FROM fx_rates f2
             WHERE f2.base_currency=f.base_currency AND f2.quote_currency=f.quote_currency
             ORDER BY f2.rate_at DESC,f2.created_at DESC LIMIT 1)"#,
    )
    .fetch_all(&state.db)
    .await?;

    let price_map: HashMap<_, _> = prices
        .into_iter()
        .map(|price| (price.instrument_id.clone(), price))
        .collect();
    let fx_map: HashMap<_, _> = rates
        .into_iter()
        .map(|rate| {
            (
                (rate.base_currency.clone(), rate.quote_currency.clone()),
                decimal(&rate.rate),
            )
        })
        .collect();
    let mut positions: HashMap<(String, String), PositionState> = HashMap::new();
    let mut metadata: HashMap<(String, String), &LedgerLeg> = HashMap::new();
    let mut income = Decimal::ZERO;
    let mut fees = Decimal::ZERO;
    let mut external_flows: Vec<(DateTime<Utc>, f64)> = Vec::new();
    let mut performance_has_missing_fx = false;

    for leg in &legs {
        let quantity = decimal(&leg.quantity);
        if leg.leg_type == "cash" {
            if let Some(fx) = fx_factor(&leg.currency, &config.report_currency, &fx_map) {
                if matches!(
                    leg.transaction_type.as_str(),
                    "dividend" | "interest" | "staking_reward" | "airdrop"
                ) && quantity > Decimal::ZERO
                {
                    income += quantity * fx;
                }
                if matches!(leg.transaction_type.as_str(), "fee" | "tax")
                    && quantity < Decimal::ZERO
                {
                    fees += -quantity * fx;
                }
                if matches!(leg.transaction_type.as_str(), "deposit" | "withdrawal")
                    && let Ok(date) = DateTime::parse_from_rfc3339(&leg.trade_at)
                {
                    let investor_flow = if leg.transaction_type == "deposit" {
                        -quantity
                    } else {
                        quantity
                    };
                    external_flows.push((
                        date.with_timezone(&Utc),
                        (investor_flow * fx).to_f64().unwrap_or(0.0),
                    ));
                }
            } else {
                performance_has_missing_fx = true;
            }
        }
        let key = (leg.account_id.clone(), leg.instrument_id.clone());
        metadata.insert(key.clone(), leg);
        let position = positions.entry(key).or_default();
        if leg.leg_type == "asset"
            && let Some(unit_price) = leg.unit_price.as_deref().map(decimal)
        {
            position.last_trade_price = Some(unit_price);
            if quantity > Decimal::ZERO {
                position.cost_basis += quantity * unit_price;
            } else if quantity < Decimal::ZERO && position.quantity > Decimal::ZERO {
                let removed_quantity = (-quantity).min(position.quantity);
                let average = position.cost_basis / position.quantity;
                let removed_cost = removed_quantity * average;
                position.cost_basis -= removed_cost;
                if leg.transaction_type == "sell" {
                    position.realized += removed_quantity * unit_price - removed_cost;
                }
            }
        }
        position.quantity += quantity;
        if position.quantity == Decimal::ZERO {
            position.cost_basis = Decimal::ZERO;
        }
    }

    let now = Utc::now();
    let mut staged = Vec::new();
    let mut total = Decimal::ZERO;
    let mut total_cost = Decimal::ZERO;
    let mut realized = Decimal::ZERO;
    for (key, position) in positions {
        if position.quantity == Decimal::ZERO {
            continue;
        }
        let meta = metadata[&key];
        let is_cash = matches!(meta.asset_type.as_str(), "cash" | "stablecoin");
        let latest = price_map.get(&meta.instrument_id);
        let market_price = if is_cash {
            Some(Decimal::ONE)
        } else {
            latest
                .map(|value| decimal(&value.price))
                .or(position.last_trade_price)
        };
        let price_currency = latest
            .map(|value| value.currency.as_str())
            .unwrap_or(&meta.currency);
        let fx = fx_factor(price_currency, &config.report_currency, &fx_map);
        let missing_fx = fx.is_none() && price_currency != config.report_currency;
        let market_value =
            market_price.unwrap_or(Decimal::ZERO) * position.quantity * fx.unwrap_or(Decimal::ZERO);
        let cost_fx = fx_factor(&meta.currency, &config.report_currency, &fx_map).unwrap_or(
            if meta.currency == config.report_currency {
                Decimal::ONE
            } else {
                Decimal::ZERO
            },
        );
        let cost_basis = position.cost_basis * cost_fx;
        let realized_pnl = position.realized * cost_fx;
        let price_at = latest.map(|value| value.price_at.clone());
        let stale = price_at
            .as_deref()
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .is_some_and(|date| {
                (now - date.with_timezone(&Utc)).num_days() > config.stale_price_days
            });
        let missing_price = !is_cash && latest.is_none();
        total += market_value;
        total_cost += cost_basis;
        realized += realized_pnl;
        staged.push((
            meta,
            position,
            market_price.unwrap_or(Decimal::ZERO),
            market_value,
            cost_basis,
            realized_pnl,
            stale,
            missing_price,
            missing_fx,
            price_at,
        ));
    }

    let mut holdings = Vec::new();
    for (
        meta,
        position,
        price,
        market_value,
        cost_basis,
        realized_pnl,
        stale,
        missing_price,
        missing_fx,
        price_at,
    ) in staged
    {
        let average_cost = if position.quantity > Decimal::ZERO {
            position.cost_basis / position.quantity
        } else {
            Decimal::ZERO
        };
        holdings.push(Holding {
            account_id: meta.account_id.clone(),
            account_name: meta.account_name.clone(),
            account_type: meta.account_type.clone(),
            instrument_id: meta.instrument_id.clone(),
            symbol: meta.symbol.clone(),
            name: meta.instrument_name.clone(),
            asset_type: meta.asset_type.clone(),
            currency: meta.currency.clone(),
            quantity: display(position.quantity),
            average_cost: display(average_cost),
            price: display(price),
            price_source: price_map
                .get(&meta.instrument_id)
                .map(|value| value.source.clone())
                .unwrap_or_else(|| {
                    if matches!(meta.asset_type.as_str(), "cash" | "stablecoin") {
                        "face_value".to_owned()
                    } else {
                        "last_transaction".to_owned()
                    }
                }),
            price_at,
            market_value: display(market_value),
            cost_basis: display(cost_basis),
            unrealized_pnl: display(market_value - cost_basis),
            realized_pnl: display(realized_pnl),
            weight: display(if total == Decimal::ZERO {
                Decimal::ZERO
            } else {
                market_value / total
            }),
            stale,
            missing_price,
            missing_fx,
        });
    }
    holdings.sort_by_key(|holding| std::cmp::Reverse(decimal(&holding.market_value)));
    let cash_value: Decimal = holdings
        .iter()
        .filter(|h| matches!(h.asset_type.as_str(), "cash" | "stablecoin"))
        .map(|h| decimal(&h.market_value))
        .sum();
    let unrealized: Decimal = holdings.iter().map(|h| decimal(&h.unrealized_pnl)).sum();
    let by_asset = allocation(&holdings, total, |h| h.asset_type.clone());
    let by_account = allocation(&holdings, total, |h| h.account_name.clone());
    let by_currency = allocation(&holdings, total, |h| h.currency.clone());
    let max_position = holdings
        .iter()
        .map(|h| decimal(&h.weight))
        .max()
        .unwrap_or(Decimal::ZERO);
    let crypto_value: Decimal = holdings
        .iter()
        .filter(|h| matches!(h.asset_type.as_str(), "crypto" | "stablecoin"))
        .map(|h| decimal(&h.market_value))
        .sum();
    let account_concentration = by_account
        .iter()
        .map(|a| decimal(&a.weight))
        .max()
        .unwrap_or(Decimal::ZERO);
    let stale_price_count = holdings.iter().filter(|holding| holding.stale).count();
    let missing_price_count = holdings
        .iter()
        .filter(|holding| holding.missing_price)
        .count();
    let missing_fx_count = holdings.iter().filter(|holding| holding.missing_fx).count();
    let targets = sqlx::query_as::<_, (String, String, String)>(
        "SELECT dimension,value,max_weight FROM targets",
    )
    .fetch_all(&state.db)
    .await?;
    let mut target_breaches = Vec::new();
    for (dimension, value, max_weight) in targets {
        let current = match dimension.as_str() {
            "asset_type" => by_asset
                .iter()
                .find(|a| a.key == value)
                .map(|a| decimal(&a.weight)),
            "currency" => by_currency
                .iter()
                .find(|a| a.key == value)
                .map(|a| decimal(&a.weight)),
            "account" => by_account
                .iter()
                .find(|a| a.key == value)
                .map(|a| decimal(&a.weight)),
            _ => None,
        }
        .unwrap_or(Decimal::ZERO);
        if current > decimal(&max_weight) {
            target_breaches.push(format!("{dimension}:{value} 超过上限"));
        }
    }
    external_flows.push((now, total.to_f64().unwrap_or(0.0)));
    let xirr_value = if performance_has_missing_fx {
        None
    } else {
        xirr(&external_flows).map(|value| format!("{value:.6}"))
    };
    Ok(PortfolioSummary {
        report_currency: config.report_currency,
        total_market_value: display(total),
        investment_value: display(total - cash_value),
        cash_value: display(cash_value),
        total_cost_basis: display(total_cost),
        holdings,
        allocation_by_asset_type: by_asset,
        allocation_by_account: by_account,
        allocation_by_currency: by_currency,
        risk: RiskSummary {
            max_position_weight: display(max_position),
            crypto_weight: display(if total == Decimal::ZERO {
                Decimal::ZERO
            } else {
                crypto_value / total
            }),
            cash_weight: display(if total == Decimal::ZERO {
                Decimal::ZERO
            } else {
                cash_value / total
            }),
            account_concentration: display(account_concentration),
            stale_price_count,
            missing_price_count,
            missing_fx_count,
            target_breaches,
        },
        performance: PerformanceSummary {
            xirr: xirr_value,
            twr: None,
            realized_pnl: display(realized),
            unrealized_pnl: display(unrealized),
            income: display(income),
            fees_and_taxes: display(fees),
            note: if performance_has_missing_fx {
                "部分现金流缺少汇率，收益、费用与 XIRR 暂不完整；补齐汇率后重新计算。".to_owned()
            } else {
                "TWR 需要连续历史估值，价格覆盖不足时不生成估算值。".to_owned()
            },
        },
        calculated_at: now.to_rfc3339(),
    })
}

fn allocation<F>(holdings: &[Holding], total: Decimal, key: F) -> Vec<AllocationItem>
where
    F: Fn(&Holding) -> String,
{
    let mut values: HashMap<String, Decimal> = HashMap::new();
    for h in holdings {
        *values.entry(key(h)).or_default() += decimal(&h.market_value);
    }
    let mut output: Vec<_> = values
        .into_iter()
        .map(|(key, value)| AllocationItem {
            key,
            value: display(value),
            weight: display(if total == Decimal::ZERO {
                Decimal::ZERO
            } else {
                value / total
            }),
        })
        .collect();
    output.sort_by_key(|item| std::cmp::Reverse(decimal(&item.value)));
    output
}
fn fx_factor(
    base: &str,
    quote: &str,
    rates: &HashMap<(String, String), Decimal>,
) -> Option<Decimal> {
    if base == quote {
        return Some(Decimal::ONE);
    }
    rates
        .get(&(base.to_owned(), quote.to_owned()))
        .copied()
        .or_else(|| {
            rates
                .get(&(quote.to_owned(), base.to_owned()))
                .and_then(|rate| {
                    if *rate == Decimal::ZERO {
                        None
                    } else {
                        Some(Decimal::ONE / *rate)
                    }
                })
        })
}
fn decimal(value: &str) -> Decimal {
    Decimal::from_str(value).unwrap_or(Decimal::ZERO)
}
fn display(value: Decimal) -> String {
    value.round_dp(8).normalize().to_string()
}
fn xirr(flows: &[(DateTime<Utc>, f64)]) -> Option<f64> {
    if flows.len() < 2
        || !flows.iter().any(|(_, v)| *v < 0.0)
        || !flows.iter().any(|(_, v)| *v > 0.0)
    {
        return None;
    }
    let start = flows.iter().map(|(d, _)| *d).min()?;
    let mut rate = 0.08_f64;
    for _ in 0..100 {
        let mut f = 0.0;
        let mut df = 0.0;
        for (date, value) in flows {
            let years = (*date - start).num_seconds() as f64 / (365.0 * 86400.0);
            let base = 1.0 + rate;
            if base <= 0.0 {
                return None;
            }
            f += value / base.powf(years);
            df -= years * value / base.powf(years + 1.0);
        }
        if df.abs() < 1e-12 {
            break;
        }
        let next = rate - f / df;
        if (next - rate).abs() < 1e-9 {
            return next.is_finite().then_some(next);
        }
        rate = next.clamp(-0.9999, 1000.0);
    }
    rate.is_finite().then_some(rate)
}
