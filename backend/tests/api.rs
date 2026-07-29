use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use investment_contracts::HEADER_INTERNAL_TOKEN;
use personal_investment_backend::{build_app_with_auth, build_service_app, db};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn test_app() -> Router {
    let pool = db::connect("sqlite::memory:").await.unwrap();
    build_app_with_auth(pool, false)
}

async fn send_json(app: &Router, method: &str, uri: &str, payload: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, body)
}

async fn create_account(app: &Router) -> String {
    let (status, body) = send_json(
        app,
        "POST",
        "/api/v1/accounts",
        json!({
            "name": "Crypto account",
            "institution": "Example Exchange",
            "account_type": "crypto_exchange",
            "base_currency": "usd",
            "include_in_net_worth": true
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["base_currency"], "USD");
    body["id"].as_str().unwrap().to_owned()
}

async fn create_instrument(app: &Router, payload: Value) -> String {
    let (status, body) = send_json(app, "POST", "/api/v1/instruments", payload).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn health_reports_database_connection() {
    let app = test_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(body["status"], "ok");
    assert_eq!(body["database"], "connected");
}

#[tokio::test]
async fn internal_core_rejects_requests_that_bypass_the_gateway() {
    let pool = db::connect("sqlite::memory:").await.unwrap();
    let app = build_service_app(pool, false, Some("test-internal-token".to_owned()));

    let rejected = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/accounts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::UNAUTHORIZED);

    let accepted = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/accounts")
                .header(HEADER_INTERNAL_TOKEN, "test-internal-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(accepted.status(), StatusCode::OK);
}

#[tokio::test]
async fn audit_exports_are_complete_filtered_and_downloadable() {
    let app = test_app().await;
    let account_id = create_account(&app).await;
    let asset_id = create_instrument(
        &app,
        json!({
            "symbol": "AUDIT",
            "name": "审计导出测试标的",
            "asset_type": "fund",
            "currency": "USD",
            "exchange": null,
            "network": null,
            "contract_address": null,
            "precision": 4
        }),
    )
    .await;
    let cash_id = create_instrument(
        &app,
        json!({
            "symbol": "USD",
            "name": "美元现金",
            "asset_type": "cash",
            "currency": "USD",
            "exchange": null,
            "network": null,
            "contract_address": null,
            "precision": 2
        }),
    )
    .await;
    let (status, created) = send_json(
        &app,
        "POST",
        "/api/v1/transactions",
        json!({
            "transaction_type": "buy",
            "trade_at": "2025-07-14T08:00:00Z",
            "settle_at": null,
            "source": "manual",
            "external_id": "audit-export-1",
            "memo": "审计导出测试",
            "legs": [
                { "account_id": account_id, "instrument_id": asset_id, "leg_type": "asset", "quantity": "2", "unit_price": "100", "price_currency": "USD", "memo": null },
                { "account_id": account_id, "instrument_id": cash_id, "leg_type": "cash", "quantity": "-200", "unit_price": null, "price_currency": null, "memo": null }
            ]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");

    let (status, summary) =
        send_json(&app, "GET", "/api/v1/audit-export/summary", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{summary}");
    assert_eq!(summary["schema_version"], "2026.07-audit-v1");
    assert_eq!(summary["counts"]["transactions"], 1);
    assert_eq!(summary["counts"]["transaction_legs"], 2);
    assert_eq!(summary["integrity"]["passed"], true);

    let (status, package) = send_json(
        &app,
        "GET",
        "/api/v1/audit-export/package?include_market=true",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{package}");
    assert_eq!(package["control_totals"]["transactions"], 1);
    assert_eq!(
        package["manifest"]["data_sha256"].as_str().unwrap().len(),
        64
    );
    assert_eq!(
        package["data"]["transaction_legs"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(!package.to_string().contains("password_hash"));

    let filtered = "/api/v1/audit-export/summary?from=2026-01-01&to=2026-12-31";
    let (status, filtered_summary) = send_json(&app, "GET", filtered, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{filtered_summary}");
    assert_eq!(filtered_summary["counts"]["transactions"], 0);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/audit-export/transactions.csv")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get("content-disposition")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("attachment")
    );
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
    let csv = String::from_utf8(bytes[3..].to_vec()).unwrap();
    assert!(csv.contains("流水ID"));
    assert!(csv.contains("审计导出测试"));

    let (status, invalid) = send_json(
        &app,
        "GET",
        "/api/v1/audit-export/summary?from=2026-12-31&to=2026-01-01",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{invalid}");
}

#[tokio::test]
async fn network_proxy_settings_support_protocols_and_validation() {
    let app = test_app().await;
    let (status, defaults) = send_json(&app, "GET", "/api/v1/network-proxy", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{defaults}");
    assert_eq!(defaults["is_enabled"], false);
    assert_eq!(defaults["protocol"], "http");

    let (status, saved) = send_json(
        &app,
        "PUT",
        "/api/v1/network-proxy",
        json!({
            "is_enabled": true,
            "protocol": "socks5",
            "host": "127.0.0.1",
            "port": 7890
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["is_enabled"], true);
    assert_eq!(saved["protocol"], "socks5");
    assert_eq!(saved["port"], 7890);

    let (status, body) = send_json(
        &app,
        "PUT",
        "/api/v1/network-proxy",
        json!({
            "is_enabled": true,
            "protocol": "http",
            "host": "http://127.0.0.1/path",
            "port": 7890
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"]["code"], "validation_error");
}

#[tokio::test]
async fn authentication_protects_private_routes() {
    let pool = db::connect("sqlite::memory:").await.unwrap();
    let app = build_app_with_auth(pool, true);

    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/accounts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let setup = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/setup")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "username": "investor",
                        "display_name": "Investor",
                        "password": "securepass123"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(setup.status(), StatusCode::CREATED);
    let cookie = setup
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();

    let authorized = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/accounts")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(authorized.status(), StatusCode::OK);
}

#[tokio::test]
async fn creates_crypto_instrument_and_rejects_duplicate_identity() {
    let app = test_app().await;
    let instrument = json!({
        "symbol": "usdt",
        "name": "Tether USD on Ethereum",
        "asset_type": "stablecoin",
        "currency": "usd",
        "exchange": null,
        "network": "ethereum",
        "contract_address": "0xdac17f958d2ee523a2206206994597c13d831ec7",
        "precision": 6
    });

    let id = create_instrument(&app, instrument.clone()).await;
    assert!(!id.is_empty());

    let (status, body) = send_json(&app, "POST", "/api/v1/instruments", instrument).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "conflict");
}

#[tokio::test]
async fn instrument_crud_supports_status_changes_and_safe_deletion() {
    let app = test_app().await;
    let id = create_instrument(
        &app,
        json!({
            "symbol": "TESTCOIN",
            "name": "Test Coin",
            "asset_type": "crypto",
            "currency": "USD",
            "exchange": "CoinGecko",
            "network": null,
            "contract_address": null,
            "precision": 8
        }),
    )
    .await;

    let (status, updated) = send_json(
        &app,
        "PUT",
        &format!("/api/v1/instruments/{id}"),
        json!({
            "symbol": "TESTCOIN",
            "name": "Updated Test Coin",
            "asset_type": "crypto",
            "currency": "USD",
            "exchange": "CoinGecko",
            "network": null,
            "contract_address": null,
            "precision": 8
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["name"], "Updated Test Coin");

    let (status, inactive) = send_json(
        &app,
        "PATCH",
        &format!("/api/v1/instruments/{id}"),
        json!({ "is_active": false }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{inactive}");
    assert_eq!(inactive["is_active"], false);

    let (status, _) = send_json(
        &app,
        "DELETE",
        &format!("/api/v1/instruments/{id}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body) = send_json(
        &app,
        "GET",
        &format!("/api/v1/instruments/{id}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

#[tokio::test]
async fn instrument_tags_support_batch_replacement_and_listing() {
    let app = test_app().await;
    let id = create_instrument(
        &app,
        json!({
            "symbol": "TAGGED",
            "name": "Tagged Instrument",
            "asset_type": "stock",
            "currency": "USD",
            "exchange": "NASDAQ",
            "network": null,
            "contract_address": null,
            "precision": 4
        }),
    )
    .await;

    let (status, tags) = send_json(
        &app,
        "PUT",
        &format!("/api/v1/instruments/{id}/tags"),
        json!({ "tags": ["核心", "美股", "核心", "  长期持有  "] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{tags}");
    assert_eq!(tags.as_array().unwrap().len(), 3);
    assert!(
        tags.as_array()
            .unwrap()
            .iter()
            .all(|tag| tag["instrument_id"] == id)
    );
    assert!(
        tags.as_array()
            .unwrap()
            .iter()
            .any(|tag| tag["name"] == "长期持有")
    );

    let (status, all_tags) = send_json(&app, "GET", "/api/v1/instrument-tags", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{all_tags}");
    assert!(
        all_tags
            .as_array()
            .unwrap()
            .iter()
            .any(|tag| { tag["instrument_id"] == id && tag["name"] == "美股" })
    );

    let (status, replaced) = send_json(
        &app,
        "PUT",
        &format!("/api/v1/instruments/{id}/tags"),
        json!({ "tags": ["卫星"] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{replaced}");
    assert_eq!(replaced.as_array().unwrap().len(), 1);
    assert_eq!(replaced[0]["name"], "卫星");

    let (status, audit_logs) = send_json(&app, "GET", "/api/v1/audit-logs", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{audit_logs}");
    assert!(
        audit_logs
            .as_array()
            .unwrap()
            .iter()
            .any(|log| { log["entity_type"] == "instrument_tags" && log["entity_id"] == id })
    );
}

#[tokio::test]
async fn planning_targets_and_decisions_support_audited_deletion() {
    let app = test_app().await;

    let (status, target) = send_json(
        &app,
        "POST",
        "/api/v1/targets",
        json!({
            "dimension": "asset_type",
            "value": "stock",
            "target_weight": "0.50",
            "min_weight": "0.40",
            "max_weight": "0.60"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{target}");
    let target_id = target["id"].as_str().unwrap();

    let (status, decision) = send_json(
        &app,
        "POST",
        "/api/v1/decisions",
        json!({
            "instrument_id": null,
            "action": "提高股票配置",
            "decided_at": "2026-07-16T10:00:00.000Z",
            "rationale": "长期目标配置出现偏离",
            "confidence": 75,
            "risks": "市场短期波动",
            "invalidation": "现金需求上升",
            "review_at": null,
            "outcome": "",
            "process_score": null,
            "result_score": null
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{decision}");
    let decision_id = decision["id"].as_str().unwrap();

    let (status, _) = send_json(
        &app,
        "DELETE",
        &format!("/api/v1/targets/{target_id}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = send_json(
        &app,
        "DELETE",
        &format!("/api/v1/decisions/{decision_id}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, missing_target) = send_json(
        &app,
        "GET",
        &format!("/api/v1/targets/{target_id}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{missing_target}");
    let (status, missing_decision) = send_json(
        &app,
        "GET",
        &format!("/api/v1/decisions/{decision_id}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{missing_decision}");

    let (status, audit_logs) = send_json(&app, "GET", "/api/v1/audit-logs", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{audit_logs}");
    assert!(audit_logs.as_array().unwrap().iter().any(|log| {
        log["entity_type"] == "target" && log["entity_id"] == target_id && log["action"] == "delete"
    }));
    assert!(audit_logs.as_array().unwrap().iter().any(|log| {
        log["entity_type"] == "decision"
            && log["entity_id"] == decision_id
            && log["action"] == "delete"
    }));
}

#[tokio::test]
async fn blockchain_networks_support_crud_and_instrument_multi_selection() {
    let app = test_app().await;
    let (status, seeded) = send_json(&app, "GET", "/api/v1/blockchain-networks", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{seeded}");
    assert!(seeded.as_array().is_some_and(|items| items.len() >= 10));

    let (status, created) = send_json(
        &app,
        "POST",
        "/api/v1/blockchain-networks",
        json!({ "code": "test-chain", "name": "Test Chain" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let network_id = created["id"].as_str().unwrap();

    let (status, updated) = send_json(
        &app,
        "PUT",
        &format!("/api/v1/blockchain-networks/{network_id}"),
        json!({ "code": "test-chain", "name": "Updated Test Chain" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["name"], "Updated Test Chain");

    let instrument_id = create_instrument(
        &app,
        json!({
            "symbol": "MULTI",
            "name": "Multi Network Token",
            "asset_type": "crypto",
            "currency": "USD",
            "exchange": null,
            "network": "ethereum, test-chain,ethereum",
            "contract_address": null,
            "precision": 8
        }),
    )
    .await;
    let (status, instrument) = send_json(
        &app,
        "GET",
        &format!("/api/v1/instruments/{instrument_id}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{instrument}");
    assert_eq!(instrument["network"], "ethereum,test-chain");

    let (status, conflict) = send_json(
        &app,
        "DELETE",
        &format!("/api/v1/blockchain-networks/{network_id}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{conflict}");

    let (status, inactive) = send_json(
        &app,
        "PATCH",
        &format!("/api/v1/blockchain-networks/{network_id}"),
        json!({ "is_active": false }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{inactive}");
    assert_eq!(inactive["is_active"], false);
}

#[tokio::test]
async fn rejects_non_major_account_base_currency() {
    let app = test_app().await;
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/v1/accounts",
        json!({
            "name": "Unsupported base currency",
            "institution": null,
            "account_type": "other",
            "base_currency": "BTC",
            "include_in_net_worth": true
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "validation_error");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("仅支持以下主流货币")
    );
}

#[tokio::test]
async fn freezes_unfreezes_and_deletes_an_empty_account() {
    let app = test_app().await;
    let account_id = create_account(&app).await;

    let (status, frozen) = send_json(
        &app,
        "PATCH",
        &format!("/api/v1/accounts/{account_id}"),
        json!({ "is_active": false }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{frozen}");
    assert_eq!(frozen["is_active"], false);

    let (status, active) = send_json(
        &app,
        "PATCH",
        &format!("/api/v1/accounts/{account_id}"),
        json!({ "is_active": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{active}");
    assert_eq!(active["is_active"], true);

    let (status, _) = send_json(
        &app,
        "DELETE",
        &format!("/api/v1/accounts/{account_id}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = send_json(
        &app,
        "GET",
        &format!("/api/v1/accounts/{account_id}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn frozen_accounts_reject_new_entries_and_used_accounts_cannot_be_deleted() {
    let app = test_app().await;
    let account_id = create_account(&app).await;
    let bitcoin_id = create_instrument(
        &app,
        json!({
            "symbol": "BTC",
            "name": "Bitcoin",
            "asset_type": "crypto",
            "currency": "USD",
            "exchange": null,
            "network": "bitcoin",
            "contract_address": null,
            "precision": 8
        }),
    )
    .await;
    let cash_id = create_instrument(
        &app,
        json!({
            "symbol": "USD",
            "name": "US Dollar",
            "asset_type": "cash",
            "currency": "USD",
            "exchange": null,
            "network": null,
            "contract_address": null,
            "precision": 2
        }),
    )
    .await;
    let payload = json!({
        "transaction_type": "buy",
        "trade_at": "2026-07-15T10:00:00+08:00",
        "source": "account-lifecycle-test",
        "external_id": "frozen-account-buy",
        "memo": null,
        "legs": [
            { "account_id": account_id, "instrument_id": bitcoin_id, "leg_type": "asset", "quantity": "1", "unit_price": "100", "price_currency": "USD", "memo": null },
            { "account_id": account_id, "instrument_id": cash_id, "leg_type": "cash", "quantity": "-100", "unit_price": null, "price_currency": null, "memo": null }
        ]
    });

    let (status, _) = send_json(
        &app,
        "PATCH",
        &format!("/api/v1/accounts/{account_id}"),
        json!({ "is_active": false }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = send_json(&app, "POST", "/api/v1/transactions", payload.clone()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("冻结账户")
    );

    let (status, _) = send_json(
        &app,
        "PATCH",
        &format!("/api/v1/accounts/{account_id}"),
        json!({ "is_active": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = send_json(&app, "POST", "/api/v1/transactions", payload).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (status, body) = send_json(
        &app,
        "DELETE",
        &format!("/api/v1/accounts/{account_id}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "conflict");
}

#[tokio::test]
async fn creates_immutable_buy_transaction_with_decimal_strings() {
    let app = test_app().await;
    let account_id = create_account(&app).await;
    let bitcoin_id = create_instrument(
        &app,
        json!({
            "symbol": "BTC",
            "name": "Bitcoin",
            "asset_type": "crypto",
            "currency": "USD",
            "exchange": null,
            "network": "bitcoin",
            "contract_address": null,
            "precision": 8
        }),
    )
    .await;
    let cash_id = create_instrument(
        &app,
        json!({
            "symbol": "USD",
            "name": "US Dollar",
            "asset_type": "cash",
            "currency": "USD",
            "exchange": null,
            "network": null,
            "contract_address": null,
            "precision": 2
        }),
    )
    .await;

    let payload = json!({
        "transaction_type": "buy",
        "trade_at": "2026-07-14T16:00:00+08:00",
        "settle_at": null,
        "source": "manual",
        "external_id": "example-buy-1",
        "memo": "initial position",
        "legs": [
            {
                "account_id": account_id,
                "instrument_id": bitcoin_id,
                "leg_type": "asset",
                "quantity": "0.12500000",
                "unit_price": "40000.00",
                "price_currency": "usd",
                "memo": null
            },
            {
                "account_id": account_id,
                "instrument_id": cash_id,
                "leg_type": "cash",
                "quantity": "-5000.00",
                "unit_price": null,
                "price_currency": null,
                "memo": null
            }
        ]
    });

    let (status, body) = send_json(&app, "POST", "/api/v1/transactions", payload.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["legs"][0]["quantity"], "0.125");
    assert_eq!(body["legs"][1]["quantity"], "-5000");
    assert_eq!(body["trade_at"], "2026-07-14T08:00:00.000Z");

    let (duplicate_status, duplicate_body) =
        send_json(&app, "POST", "/api/v1/transactions", payload).await;
    assert_eq!(duplicate_status, StatusCode::CONFLICT);
    assert_eq!(duplicate_body["error"]["code"], "conflict");
}

#[tokio::test]
async fn rejects_buy_without_cash_leg() {
    let app = test_app().await;
    let account_id = create_account(&app).await;
    let bitcoin_id = create_instrument(
        &app,
        json!({
            "symbol": "BTC",
            "name": "Bitcoin",
            "asset_type": "crypto",
            "currency": "USD",
            "exchange": null,
            "network": "bitcoin",
            "contract_address": null,
            "precision": 8
        }),
    )
    .await;

    let (status, body) = send_json(
        &app,
        "POST",
        "/api/v1/transactions",
        json!({
            "transaction_type": "buy",
            "trade_at": "2026-07-14T08:00:00Z",
            "legs": [{
                "account_id": account_id,
                "instrument_id": bitcoin_id,
                "leg_type": "asset",
                "quantity": "1",
                "unit_price": "1",
                "price_currency": "USD"
            }]
        }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "validation_error");
}

#[tokio::test]
async fn warns_about_semantically_duplicate_transactions_without_external_ids() {
    let app = test_app().await;
    let account_id = create_account(&app).await;
    let cash_id = create_instrument(
        &app,
        json!({
            "symbol": "USD",
            "name": "US Dollar",
            "asset_type": "cash",
            "currency": "USD",
            "exchange": null,
            "network": null,
            "contract_address": null,
            "precision": 2
        }),
    )
    .await;
    let payload = json!({
        "transaction_type": "deposit",
        "trade_at": "2026-07-14T08:00:00Z",
        "source": "web_standard",
        "memo": "salary transfer",
        "legs": [{
            "account_id": account_id,
            "instrument_id": cash_id,
            "leg_type": "cash",
            "quantity": "2500"
        }]
    });

    let (before_status, before) = send_json(
        &app,
        "POST",
        "/api/v1/transactions/duplicate-check",
        payload.clone(),
    )
    .await;
    assert_eq!(before_status, StatusCode::OK, "{before}");
    assert_eq!(before["duplicate"], false);

    let (create_status, created) = send_json(&app, "POST", "/api/v1/transactions", payload).await;
    assert_eq!(create_status, StatusCode::CREATED, "{created}");

    let (duplicate_status, duplicate) = send_json(
        &app,
        "POST",
        "/api/v1/transactions/duplicate-check",
        json!({
            "transaction_type": "deposit",
            "trade_at": "2026-07-14T09:00:00Z",
            "source": "web_standard",
            "legs": [{
                "account_id": account_id,
                "instrument_id": cash_id,
                "leg_type": "cash",
                "quantity": "2500.00"
            }]
        }),
    )
    .await;
    assert_eq!(duplicate_status, StatusCode::OK, "{duplicate}");
    assert_eq!(duplicate["duplicate"], true);
    assert_eq!(duplicate["matches"][0]["id"], created["id"]);

    let (different_status, different) = send_json(
        &app,
        "POST",
        "/api/v1/transactions/duplicate-check",
        json!({
            "transaction_type": "deposit",
            "trade_at": "2026-07-14T09:00:00Z",
            "source": "web_standard",
            "legs": [{
                "account_id": account_id,
                "instrument_id": cash_id,
                "leg_type": "cash",
                "quantity": "2600"
            }]
        }),
    )
    .await;
    assert_eq!(different_status, StatusCode::OK, "{different}");
    assert_eq!(different["duplicate"], false);
}

#[tokio::test]
async fn edits_transaction_by_reversing_and_replacing_it() {
    let app = test_app().await;
    let account_id = create_account(&app).await;
    let asset_id = create_instrument(
        &app,
        json!({
            "symbol": "VT",
            "name": "Vanguard Total World Stock ETF",
            "asset_type": "etf",
            "currency": "USD",
            "exchange": "NYSE",
            "network": null,
            "contract_address": null,
            "precision": 4
        }),
    )
    .await;
    let cash_id = create_instrument(
        &app,
        json!({
            "symbol": "USD",
            "name": "US Dollar",
            "asset_type": "cash",
            "currency": "USD",
            "exchange": null,
            "network": null,
            "contract_address": null,
            "precision": 2
        }),
    )
    .await;

    let original_payload = json!({
        "transaction_type": "buy",
        "trade_at": "2026-07-14T08:00:00Z",
        "source": "web",
        "memo": "original",
        "legs": [
            { "account_id": account_id, "instrument_id": asset_id, "leg_type": "asset", "quantity": "10", "unit_price": "100", "price_currency": "USD" },
            { "account_id": account_id, "instrument_id": cash_id, "leg_type": "cash", "quantity": "-1000" }
        ]
    });
    let (create_status, created) =
        send_json(&app, "POST", "/api/v1/transactions", original_payload).await;
    assert_eq!(create_status, StatusCode::CREATED);
    let original_id = created["id"].as_str().unwrap();

    let replacement_payload = json!({
        "transaction_type": "buy",
        "trade_at": "2026-07-14T08:30:00Z",
        "source": "web_correction",
        "memo": "corrected",
        "legs": [
            { "account_id": account_id, "instrument_id": asset_id, "leg_type": "asset", "quantity": "12", "unit_price": "105", "price_currency": "USD" },
            { "account_id": account_id, "instrument_id": cash_id, "leg_type": "cash", "quantity": "-1260" }
        ]
    });
    let (replace_status, replaced) = send_json(
        &app,
        "PUT",
        &format!("/api/v1/transactions/{original_id}"),
        replacement_payload,
    )
    .await;
    assert_eq!(replace_status, StatusCode::OK, "{replaced}");
    assert_ne!(replaced["id"], created["id"]);
    assert_eq!(replaced["legs"][0]["quantity"], "12");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/transactions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let listed: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["id"], replaced["id"]);

    let (second_status, second_body) = send_json(
        &app,
        "PUT",
        &format!("/api/v1/transactions/{original_id}"),
        json!({
            "transaction_type": "buy",
            "trade_at": "2026-07-14T08:30:00Z",
            "legs": [
                { "account_id": account_id, "instrument_id": asset_id, "leg_type": "asset", "quantity": "12", "unit_price": "105", "price_currency": "USD" },
                { "account_id": account_id, "instrument_id": cash_id, "leg_type": "cash", "quantity": "-1260" }
            ]
        }),
    )
    .await;
    assert_eq!(second_status, StatusCode::CONFLICT);
    assert_eq!(second_body["error"]["code"], "conflict");
}

#[tokio::test]
async fn reads_and_deletes_transaction_by_creating_an_auditable_void() {
    let app = test_app().await;
    let account_id = create_account(&app).await;
    let cash_id = create_instrument(
        &app,
        json!({
            "symbol": "USD",
            "name": "US Dollar",
            "asset_type": "cash",
            "currency": "USD",
            "exchange": null,
            "network": null,
            "contract_address": null,
            "precision": 2
        }),
    )
    .await;

    let (create_status, created) = send_json(
        &app,
        "POST",
        "/api/v1/transactions",
        json!({
            "transaction_type": "deposit",
            "trade_at": "2026-07-14T08:00:00Z",
            "source": "web",
            "memo": "temporary deposit",
            "legs": [{
                "account_id": account_id,
                "instrument_id": cash_id,
                "leg_type": "cash",
                "quantity": "2500"
            }]
        }),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED, "{created}");
    let transaction_id = created["id"].as_str().unwrap();

    let (read_status, fetched) = send_json(
        &app,
        "GET",
        &format!("/api/v1/transactions/{transaction_id}"),
        Value::Null,
    )
    .await;
    assert_eq!(read_status, StatusCode::OK, "{fetched}");
    assert_eq!(fetched["memo"], "temporary deposit");
    assert_eq!(fetched["legs"][0]["quantity"], "2500");

    let (delete_status, delete_body) = send_json(
        &app,
        "DELETE",
        &format!("/api/v1/transactions/{transaction_id}"),
        Value::Null,
    )
    .await;
    assert_eq!(delete_status, StatusCode::NO_CONTENT, "{delete_body}");

    let (read_void_status, voided) = send_json(
        &app,
        "GET",
        &format!("/api/v1/transactions/{transaction_id}"),
        Value::Null,
    )
    .await;
    assert_eq!(read_void_status, StatusCode::OK, "{voided}");
    assert_eq!(voided["status"], "reversed");

    let listed_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/transactions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let listed: Value = serde_json::from_slice(
        &to_bytes(listed_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(listed.as_array().unwrap().is_empty());

    let (second_delete_status, second_delete_body) = send_json(
        &app,
        "DELETE",
        &format!("/api/v1/transactions/{transaction_id}"),
        Value::Null,
    )
    .await;
    assert_eq!(second_delete_status, StatusCode::CONFLICT);
    assert_eq!(second_delete_body["error"]["code"], "conflict");
}

#[tokio::test]
async fn permanently_deletes_only_recent_manual_transactions_when_enabled() {
    let app = test_app().await;
    let account_id = create_account(&app).await;
    let cash_id = create_instrument(
        &app,
        json!({
            "symbol": "CNY-CORRECTION",
            "name": "Correction Cash",
            "asset_type": "cash",
            "currency": "CNY",
            "exchange": null,
            "network": null,
            "contract_address": null,
            "precision": 2
        }),
    )
    .await;

    let (create_status, created) = send_json(
        &app,
        "POST",
        "/api/v1/transactions",
        json!({
            "transaction_type": "deposit",
            "trade_at": "2026-07-16T10:00:00Z",
            "source": "web_standard",
            "memo": "mistyped entry",
            "legs": [{
                "account_id": account_id,
                "instrument_id": cash_id,
                "leg_type": "cash",
                "quantity": "999"
            }]
        }),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED, "{created}");
    let transaction_id = created["id"].as_str().unwrap();

    let (delete_status, delete_body) = send_json(
        &app,
        "DELETE",
        &format!("/api/v1/transactions/{transaction_id}/permanent"),
        Value::Null,
    )
    .await;
    assert_eq!(delete_status, StatusCode::NO_CONTENT, "{delete_body}");

    let (read_status, read_body) = send_json(
        &app,
        "GET",
        &format!("/api/v1/transactions/{transaction_id}"),
        Value::Null,
    )
    .await;
    assert_eq!(read_status, StatusCode::NOT_FOUND, "{read_body}");

    let (package_status, package) =
        send_json(&app, "GET", "/api/v1/audit-export/package", Value::Null).await;
    assert_eq!(package_status, StatusCode::OK, "{package}");
    assert!(
        package["data"]["audit_logs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["entity_id"] == transaction_id && entry["action"] == "hard_delete")
    );

    let (settings_status, settings) = send_json(
        &app,
        "PUT",
        "/api/v1/settings",
        json!({
            "report_currency": "CNY",
            "timezone": "Asia/Shanghai",
            "cost_method": "average",
            "stale_price_days": 3,
            "absolute_rebalance_threshold": "0.04",
            "relative_rebalance_threshold": "0.20",
            "transaction_hard_delete_minutes": 0
        }),
    )
    .await;
    assert_eq!(settings_status, StatusCode::OK, "{settings}");
    assert_eq!(settings["transaction_hard_delete_minutes"], 0);

    let (_, second) = send_json(
        &app,
        "POST",
        "/api/v1/transactions",
        json!({
            "transaction_type": "deposit",
            "trade_at": "2026-07-16T10:05:00Z",
            "source": "web",
            "legs": [{
                "account_id": account_id,
                "instrument_id": cash_id,
                "leg_type": "cash",
                "quantity": "100"
            }]
        }),
    )
    .await;
    let second_id = second["id"].as_str().unwrap();
    let (disabled_status, disabled_body) = send_json(
        &app,
        "DELETE",
        &format!("/api/v1/transactions/{second_id}/permanent"),
        Value::Null,
    )
    .await;
    assert_eq!(disabled_status, StatusCode::CONFLICT, "{disabled_body}");
    assert_eq!(disabled_body["error"]["code"], "conflict");
}

#[tokio::test]
async fn api_collectors_support_create_read_update_and_archive_delete() {
    let app = test_app().await;
    let instrument_id = create_instrument(
        &app,
        json!({
            "symbol": "BTC",
            "name": "Bitcoin",
            "asset_type": "crypto",
            "currency": "CNY",
            "exchange": null,
            "network": "bitcoin",
            "contract_address": null,
            "precision": 8
        }),
    )
    .await;
    let create_payload = json!({
        "name": "BTC 公共价格",
        "source_type": "market_data",
        "priority": 100,
        "config": {
            "provider": "coingecko_simple",
            "instrument_id": instrument_id,
            "coin_id": "bitcoin",
            "vs_currency": "cny"
        },
        "data_type": "prices",
        "interval_seconds": 3600,
        "timezone": "Asia/Shanghai",
        "is_enabled": true
    });
    let (create_status, created) = send_json(
        &app,
        "POST",
        "/api/v1/api-collectors",
        create_payload.clone(),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED, "{created}");
    assert_eq!(created["name"], "BTC 公共价格");
    assert_eq!(created["is_enabled"], true);
    let id = created["id"].as_str().unwrap();

    let (get_status, fetched) = send_json(
        &app,
        "GET",
        &format!("/api/v1/api-collectors/{id}"),
        Value::Null,
    )
    .await;
    assert_eq!(get_status, StatusCode::OK);
    assert_eq!(fetched["config"]["coin_id"], "bitcoin");

    let mut update_payload = create_payload;
    update_payload["name"] = json!("BTC 每日价格");
    update_payload["interval_seconds"] = json!(86400);
    update_payload["is_enabled"] = json!(false);
    let (update_status, updated) = send_json(
        &app,
        "PUT",
        &format!("/api/v1/api-collectors/{id}"),
        update_payload,
    )
    .await;
    assert_eq!(update_status, StatusCode::OK, "{updated}");
    assert_eq!(updated["name"], "BTC 每日价格");
    assert_eq!(updated["interval_seconds"], 86400);
    assert_eq!(updated["is_enabled"], false);

    let (delete_status, _) = send_json(
        &app,
        "DELETE",
        &format!("/api/v1/api-collectors/{id}"),
        Value::Null,
    )
    .await;
    assert_eq!(delete_status, StatusCode::NO_CONTENT);
    let (missing_status, _) = send_json(
        &app,
        "GET",
        &format!("/api/v1/api-collectors/{id}"),
        Value::Null,
    )
    .await;
    assert_eq!(missing_status, StatusCode::NOT_FOUND);
    let (list_status, listed) = send_json(&app, "GET", "/api/v1/api-collectors", Value::Null).await;
    assert_eq!(list_status, StatusCode::OK);
    assert!(listed.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn api_collectors_encrypt_redact_preserve_and_clear_api_keys() {
    let app = test_app().await;
    let payload = json!({
        "name": "Authenticated FX API",
        "source_type": "fx",
        "priority": 80,
        "config": {
            "provider": "generic_json",
            "url": "https://example.com/rates?base={base}&quote={quote}",
            "value_path": "data.rate",
            "base": "USD",
            "quote": "CNY",
            "auth_type": "header",
            "api_key_name": "X-API-Key"
        },
        "data_type": "fx_rates",
        "interval_seconds": 3600,
        "timezone": "Asia/Shanghai",
        "is_enabled": true,
        "api_key": "top-secret-test-key"
    });
    let (status, created) =
        send_json(&app, "POST", "/api/v1/api-collectors", payload.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["has_api_key"], true);
    assert!(created["config"].get("api_key").is_none());
    assert!(!created.to_string().contains("top-secret-test-key"));
    let id = created["id"].as_str().unwrap();

    let mut preserve_payload = payload.clone();
    preserve_payload.as_object_mut().unwrap().remove("api_key");
    let (status, preserved) = send_json(
        &app,
        "PUT",
        &format!("/api/v1/api-collectors/{id}"),
        preserve_payload,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preserved}");
    assert_eq!(preserved["has_api_key"], true);

    let mut clear_payload = payload;
    clear_payload.as_object_mut().unwrap().remove("api_key");
    clear_payload["clear_api_key"] = json!(true);
    let (status, cleared) = send_json(
        &app,
        "PUT",
        &format!("/api/v1/api-collectors/{id}"),
        clear_payload,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cleared}");
    assert_eq!(cleared["has_api_key"], false);
}

#[tokio::test]
async fn collector_connection_test_requires_key_before_network_request() {
    let app = test_app().await;
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/v1/api-collectors/test",
        json!({
            "name": "FX test",
            "data_type": "fx_rates",
            "config": {
                "provider": "generic_json",
                "url": "https://example.com/rates",
                "value_path": "data.rate",
                "base": "USD",
                "quote": "CNY",
                "auth_type": "header",
                "api_key_name": "X-API-Key"
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("API Key")
    );
}
