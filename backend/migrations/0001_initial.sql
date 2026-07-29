PRAGMA foreign_keys = ON;

CREATE TABLE accounts (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    institution TEXT,
    account_type TEXT NOT NULL CHECK (account_type IN (
        'brokerage', 'bank', 'fund_platform', 'pension',
        'crypto_exchange', 'self_custody_wallet', 'other'
    )),
    base_currency TEXT NOT NULL CHECK (length(base_currency) BETWEEN 2 AND 16),
    include_in_net_worth INTEGER NOT NULL DEFAULT 1 CHECK (include_in_net_worth IN (0, 1)),
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE instruments (
    id TEXT PRIMARY KEY NOT NULL,
    symbol TEXT NOT NULL CHECK (length(trim(symbol)) > 0),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    asset_type TEXT NOT NULL CHECK (asset_type IN (
        'stock', 'etf', 'fund', 'bond', 'cash', 'deposit',
        'gold', 'crypto', 'stablecoin', 'other'
    )),
    currency TEXT NOT NULL CHECK (length(currency) BETWEEN 2 AND 16),
    exchange TEXT,
    network TEXT,
    contract_address TEXT,
    precision INTEGER CHECK (precision BETWEEN 0 AND 30),
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE UNIQUE INDEX idx_instruments_identity
ON instruments (
    upper(symbol),
    ifnull(upper(exchange), ''),
    ifnull(upper(network), ''),
    ifnull(lower(contract_address), '')
);

CREATE TABLE transactions (
    id TEXT PRIMARY KEY NOT NULL,
    transaction_type TEXT NOT NULL CHECK (transaction_type IN (
        'buy', 'sell', 'deposit', 'withdrawal', 'transfer',
        'dividend', 'interest', 'return_of_capital', 'fee', 'tax',
        'staking_reward', 'airdrop', 'corporate_action', 'adjustment', 'valuation'
    )),
    trade_at TEXT NOT NULL,
    settle_at TEXT,
    source TEXT NOT NULL DEFAULT 'manual' CHECK (length(trim(source)) > 0),
    external_id TEXT,
    memo TEXT,
    status TEXT NOT NULL DEFAULT 'confirmed' CHECK (status IN ('confirmed', 'reversed')),
    reverses_transaction_id TEXT REFERENCES transactions(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE UNIQUE INDEX idx_transactions_external_id
ON transactions (source, external_id)
WHERE external_id IS NOT NULL;

CREATE INDEX idx_transactions_trade_at ON transactions(trade_at);

CREATE TABLE transaction_legs (
    id TEXT PRIMARY KEY NOT NULL,
    transaction_id TEXT NOT NULL REFERENCES transactions(id) ON DELETE RESTRICT,
    sequence INTEGER NOT NULL CHECK (sequence >= 0),
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    instrument_id TEXT NOT NULL REFERENCES instruments(id) ON DELETE RESTRICT,
    leg_type TEXT NOT NULL CHECK (leg_type IN ('asset', 'cash', 'fee', 'tax', 'income', 'other')),
    quantity TEXT NOT NULL CHECK (length(trim(quantity)) > 0),
    unit_price TEXT,
    price_currency TEXT,
    memo TEXT,
    UNIQUE (transaction_id, sequence)
);

CREATE INDEX idx_transaction_legs_account ON transaction_legs(account_id);
CREATE INDEX idx_transaction_legs_instrument ON transaction_legs(instrument_id);

CREATE TABLE prices (
    instrument_id TEXT NOT NULL REFERENCES instruments(id) ON DELETE RESTRICT,
    price_at TEXT NOT NULL,
    price TEXT NOT NULL,
    currency TEXT NOT NULL,
    source TEXT NOT NULL,
    is_manual_override INTEGER NOT NULL DEFAULT 0 CHECK (is_manual_override IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (instrument_id, price_at, source)
);

CREATE TABLE fx_rates (
    base_currency TEXT NOT NULL,
    quote_currency TEXT NOT NULL,
    rate_at TEXT NOT NULL,
    rate TEXT NOT NULL,
    source TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (base_currency, quote_currency, rate_at, source),
    CHECK (base_currency <> quote_currency)
);

CREATE TABLE import_batches (
    id TEXT PRIMARY KEY NOT NULL,
    source TEXT NOT NULL,
    imported_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    checksum TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('preview', 'confirmed', 'failed')),
    stats_json TEXT NOT NULL DEFAULT '{}',
    UNIQUE (source, checksum)
);

CREATE TABLE data_sources (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    source_type TEXT NOT NULL CHECK (source_type IN (
        'market_data', 'fx', 'benchmark', 'broker', 'crypto_exchange', 'blockchain'
    )),
    priority INTEGER NOT NULL DEFAULT 100,
    credentials_ref TEXT,
    config_json TEXT NOT NULL DEFAULT '{}',
    is_enabled INTEGER NOT NULL DEFAULT 0 CHECK (is_enabled IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE sync_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    data_source_id TEXT NOT NULL REFERENCES data_sources(id) ON DELETE RESTRICT,
    name TEXT NOT NULL,
    data_type TEXT NOT NULL CHECK (data_type IN ('prices', 'fx_rates', 'balances', 'transactions')),
    interval_seconds INTEGER NOT NULL CHECK (interval_seconds >= 60),
    timezone TEXT NOT NULL DEFAULT 'UTC',
    cursor TEXT,
    retry_policy_json TEXT NOT NULL DEFAULT '{}',
    next_run_at TEXT,
    last_run_at TEXT,
    is_enabled INTEGER NOT NULL DEFAULT 0 CHECK (is_enabled IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE sync_runs (
    id TEXT PRIMARY KEY NOT NULL,
    job_id TEXT NOT NULL REFERENCES sync_jobs(id) ON DELETE RESTRICT,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed', 'partial')),
    stats_json TEXT NOT NULL DEFAULT '{}',
    error_message TEXT
);

CREATE TABLE staged_records (
    id TEXT PRIMARY KEY NOT NULL,
    sync_run_id TEXT NOT NULL REFERENCES sync_runs(id) ON DELETE RESTRICT,
    external_id TEXT,
    payload_hash TEXT NOT NULL,
    normalized_data_json TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'accepted', 'rejected', 'duplicate')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (sync_run_id, payload_hash)
);

CREATE TABLE audit_logs (
    id TEXT PRIMARY KEY NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    action TEXT NOT NULL,
    before_json TEXT,
    after_json TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_audit_logs_entity ON audit_logs(entity_type, entity_id, created_at);
