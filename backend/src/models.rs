use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub institution: Option<String>,
    pub account_type: String,
    pub base_currency: String,
    pub include_in_net_worth: bool,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Instrument {
    pub id: String,
    pub symbol: String,
    pub name: String,
    pub asset_type: String,
    pub currency: String,
    pub exchange: Option<String>,
    pub network: Option<String>,
    pub contract_address: Option<String>,
    pub precision: Option<i64>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct BlockchainNetwork {
    pub id: String,
    pub code: String,
    pub name: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct TransactionRecord {
    pub id: String,
    pub transaction_type: String,
    pub trade_at: String,
    pub settle_at: Option<String>,
    pub source: String,
    pub external_id: Option<String>,
    pub memo: Option<String>,
    pub status: String,
    pub reverses_transaction_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct TransactionLeg {
    pub id: String,
    pub transaction_id: String,
    pub sequence: i64,
    pub account_id: String,
    pub instrument_id: String,
    pub leg_type: String,
    pub quantity: String,
    pub unit_price: Option<String>,
    pub price_currency: Option<String>,
    pub memo: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransactionWithLegs {
    #[serde(flatten)]
    pub transaction: TransactionRecord,
    pub legs: Vec<TransactionLeg>,
}
