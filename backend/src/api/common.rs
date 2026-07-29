use crate::error::{AppError, AppResult};
use chrono::{DateTime, SecondsFormat, Utc};
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

pub const MAJOR_CURRENCIES: [&str; 11] = [
    "CNY", "USD", "EUR", "GBP", "JPY", "HKD", "CHF", "CAD", "AUD", "SGD", "NZD",
];

pub fn required_text(value: &str, field: &str, max_len: usize) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::Validation(format!("{field} 不能为空")));
    }
    if value.chars().count() > max_len {
        return Err(AppError::Validation(format!(
            "{field} 不能超过 {max_len} 个字符"
        )));
    }
    Ok(value.to_owned())
}

pub fn optional_text(
    value: Option<&str>,
    field: &str,
    max_len: usize,
) -> AppResult<Option<String>> {
    value
        .map(|value| required_text(value, field, max_len))
        .transpose()
}

pub fn currency(value: &str, field: &str) -> AppResult<String> {
    let value = required_text(value, field, 16)?.to_ascii_uppercase();
    if value.len() < 2
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        return Err(AppError::Validation(format!(
            "{field} 必须是 2 至 16 位字母或数字"
        )));
    }
    Ok(value)
}

pub fn major_currency(value: &str, field: &str) -> AppResult<String> {
    let value = currency(value, field)?;
    if !MAJOR_CURRENCIES.contains(&value.as_str()) {
        return Err(AppError::Validation(format!(
            "{field} 仅支持以下主流货币：{}",
            MAJOR_CURRENCIES.join("、")
        )));
    }
    Ok(value)
}

pub fn id(value: &str, field: &str) -> AppResult<String> {
    Uuid::parse_str(value)
        .map(|id| id.to_string())
        .map_err(|_| AppError::Validation(format!("{field} 不是有效的 UUID")))
}

pub fn decimal(value: &str, field: &str, allow_zero: bool) -> AppResult<String> {
    let value = Decimal::from_str(value.trim())
        .map_err(|_| AppError::Validation(format!("{field} 不是有效的十进制数")))?;
    if !allow_zero && value == Decimal::ZERO {
        return Err(AppError::Validation(format!("{field} 不能为 0")));
    }
    Ok(value.normalize().to_string())
}

pub fn positive_decimal(value: &str, field: &str) -> AppResult<String> {
    let normalized = decimal(value, field, false)?;
    let parsed = Decimal::from_str(&normalized).expect("normalized decimal must parse");
    if parsed <= Decimal::ZERO {
        return Err(AppError::Validation(format!("{field} 必须大于 0")));
    }
    Ok(normalized)
}

pub fn rfc3339(value: &str, field: &str) -> AppResult<String> {
    DateTime::parse_from_rfc3339(value.trim())
        .map(|value| {
            value
                .with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Millis, true)
        })
        .map_err(|_| AppError::Validation(format!("{field} 必须是 RFC 3339 时间")))
}

pub fn is_unique_violation(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(error) => error.is_unique_violation(),
        _ => false,
    }
}
