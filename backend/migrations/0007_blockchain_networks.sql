CREATE TABLE blockchain_networks (
    id TEXT PRIMARY KEY NOT NULL,
    code TEXT NOT NULL COLLATE NOCASE CHECK (length(trim(code)) BETWEEN 1 AND 32),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 80),
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (code)
);

INSERT INTO blockchain_networks (id, code, name) VALUES
    ('00000000-0000-4000-8000-000000000001', 'bitcoin', 'Bitcoin'),
    ('00000000-0000-4000-8000-000000000002', 'ethereum', 'Ethereum'),
    ('00000000-0000-4000-8000-000000000003', 'bnb-smart-chain', 'BNB Smart Chain'),
    ('00000000-0000-4000-8000-000000000004', 'solana', 'Solana'),
    ('00000000-0000-4000-8000-000000000005', 'tron', 'TRON'),
    ('00000000-0000-4000-8000-000000000006', 'polygon', 'Polygon PoS'),
    ('00000000-0000-4000-8000-000000000007', 'arbitrum-one', 'Arbitrum One'),
    ('00000000-0000-4000-8000-000000000008', 'optimism', 'Optimism'),
    ('00000000-0000-4000-8000-000000000009', 'avalanche-c-chain', 'Avalanche C-Chain'),
    ('00000000-0000-4000-8000-000000000010', 'base', 'Base');
