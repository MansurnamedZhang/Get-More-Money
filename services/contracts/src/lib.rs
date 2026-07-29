use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const API_VERSION: &str = "v1";
pub const HEADER_CORRELATION_ID: &str = "x-correlation-id";
pub const HEADER_ACTOR_ID: &str = "x-actor-id";
pub const HEADER_ACTOR_NAME: &str = "x-actor-name";
pub const HEADER_INTERNAL_TOKEN: &str = "x-internal-token";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActorContext {
    pub id: String,
    pub username: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope<T = Value> {
    pub event_id: Uuid,
    pub event_type: String,
    pub event_version: u16,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Uuid,
    pub causation_id: Option<Uuid>,
    pub actor: Option<ActorContext>,
    pub payload: T,
}

impl<T> EventEnvelope<T> {
    pub fn new(
        event_type: impl Into<String>,
        aggregate_type: impl Into<String>,
        aggregate_id: impl Into<String>,
        correlation_id: Uuid,
        actor: Option<ActorContext>,
        payload: T,
    ) -> Self {
        Self {
            event_id: Uuid::now_v7(),
            event_type: event_type.into(),
            event_version: 1,
            aggregate_type: aggregate_type.into(),
            aggregate_id: aggregate_id.into(),
            occurred_at: Utc::now(),
            correlation_id,
            causation_id: None,
            actor,
            payload,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceTarget {
    Identity,
    InvestmentCore,
    MarketData,
    Planning,
    Audit,
}

pub fn classify_api_path(path: &str) -> ServiceTarget {
    let path = path.strip_prefix("/api/v1").unwrap_or(path);

    if path == "/auth" || path.starts_with("/auth/") {
        return ServiceTarget::Identity;
    }
    if starts_with_any(
        path,
        &[
            "/prices",
            "/fx-rates",
            "/data-sources",
            "/sync-jobs",
            "/sync-runs",
            "/api-collectors",
            "/blockchain-networks",
            "/network-proxy",
        ],
    ) {
        return ServiceTarget::MarketData;
    }
    if starts_with_any(
        path,
        &["/policy", "/targets", "/decisions", "/reviews", "/theses"],
    ) {
        return ServiceTarget::Planning;
    }
    if starts_with_any(
        path,
        &[
            "/audit-export",
            "/audit-logs",
            "/reconciliations",
            "/classifications",
        ],
    ) {
        return ServiceTarget::Audit;
    }
    ServiceTarget::InvestmentCore
}

fn starts_with_any(path: &str, prefixes: &[&str]) -> bool {
    prefixes
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(&format!("{prefix}/")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_domains_without_frontend_knowledge() {
        assert_eq!(
            classify_api_path("/api/v1/auth/status"),
            ServiceTarget::Identity
        );
        assert_eq!(
            classify_api_path("/api/v1/transactions"),
            ServiceTarget::InvestmentCore
        );
        assert_eq!(
            classify_api_path("/api/v1/api-collectors/abc/run"),
            ServiceTarget::MarketData
        );
        assert_eq!(
            classify_api_path("/api/v1/decisions"),
            ServiceTarget::Planning
        );
        assert_eq!(
            classify_api_path("/api/v1/audit-export/summary"),
            ServiceTarget::Audit
        );
    }
}
