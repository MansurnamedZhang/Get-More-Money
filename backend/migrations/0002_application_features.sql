CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL UNIQUE COLLATE NOCASE,
    display_name TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE auth_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_seen_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_auth_sessions_expiry ON auth_sessions(expires_at);

CREATE TABLE app_settings (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    report_currency TEXT NOT NULL DEFAULT 'CNY',
    timezone TEXT NOT NULL DEFAULT 'Asia/Shanghai',
    cost_method TEXT NOT NULL DEFAULT 'average' CHECK (cost_method IN ('average', 'fifo')),
    stale_price_days INTEGER NOT NULL DEFAULT 3 CHECK (stale_price_days >= 0),
    absolute_rebalance_threshold TEXT NOT NULL DEFAULT '0.05',
    relative_rebalance_threshold TEXT NOT NULL DEFAULT '0.25',
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT INTO app_settings (id) VALUES (1);

CREATE TABLE classifications (
    id TEXT PRIMARY KEY NOT NULL,
    instrument_id TEXT NOT NULL REFERENCES instruments(id) ON DELETE CASCADE,
    dimension TEXT NOT NULL,
    value TEXT NOT NULL,
    valid_from TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (instrument_id, dimension, valid_from)
);

CREATE TABLE targets (
    id TEXT PRIMARY KEY NOT NULL,
    dimension TEXT NOT NULL,
    value TEXT NOT NULL,
    target_weight TEXT NOT NULL,
    min_weight TEXT NOT NULL,
    max_weight TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (dimension, value)
);

CREATE TABLE investment_policy (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    objective TEXT NOT NULL DEFAULT '',
    horizon_years INTEGER NOT NULL DEFAULT 10,
    max_drawdown TEXT NOT NULL DEFAULT '0.20',
    max_single_position TEXT NOT NULL DEFAULT '0.30',
    max_high_risk TEXT NOT NULL DEFAULT '0.15',
    emergency_fund_months INTEGER NOT NULL DEFAULT 12,
    allowed_tools TEXT NOT NULL DEFAULT '股票,ETF,基金,债券,现金,黄金,虚拟货币',
    prohibited_tools TEXT NOT NULL DEFAULT '融资融券,期权,期货,自动交易',
    rebalance_frequency TEXT NOT NULL DEFAULT 'quarterly',
    review_frequency TEXT NOT NULL DEFAULT 'monthly',
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT INTO investment_policy (id) VALUES (1);

CREATE TABLE investment_theses (
    id TEXT PRIMARY KEY NOT NULL,
    instrument_id TEXT NOT NULL REFERENCES instruments(id) ON DELETE CASCADE,
    thesis TEXT NOT NULL,
    risks TEXT NOT NULL DEFAULT '',
    invalidation TEXT NOT NULL DEFAULT '',
    review_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE decision_logs (
    id TEXT PRIMARY KEY NOT NULL,
    instrument_id TEXT REFERENCES instruments(id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    decided_at TEXT NOT NULL,
    rationale TEXT NOT NULL,
    confidence INTEGER NOT NULL DEFAULT 50 CHECK (confidence BETWEEN 0 AND 100),
    risks TEXT NOT NULL DEFAULT '',
    invalidation TEXT NOT NULL DEFAULT '',
    review_at TEXT,
    outcome TEXT NOT NULL DEFAULT '',
    process_score INTEGER CHECK (process_score BETWEEN 0 AND 100),
    result_score INTEGER CHECK (result_score BETWEEN 0 AND 100),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE reviews (
    id TEXT PRIMARY KEY NOT NULL,
    period_type TEXT NOT NULL CHECK (period_type IN ('weekly', 'monthly', 'quarterly', 'annual')),
    period_start TEXT NOT NULL,
    period_end TEXT NOT NULL,
    summary TEXT NOT NULL,
    actions TEXT NOT NULL DEFAULT '',
    completed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE reconciliations (
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    reconciled_at TEXT NOT NULL,
    statement_balance TEXT NOT NULL,
    ledger_balance TEXT NOT NULL,
    difference TEXT NOT NULL,
    note TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_prices_latest ON prices(instrument_id, price_at DESC);
CREATE INDEX idx_fx_rates_latest ON fx_rates(base_currency, quote_currency, rate_at DESC);
CREATE INDEX idx_decisions_review ON decision_logs(review_at);
CREATE INDEX idx_reviews_period ON reviews(period_start, period_end);
