CREATE TABLE api_credentials (
    data_source_id TEXT PRIMARY KEY NOT NULL REFERENCES data_sources(id) ON DELETE CASCADE,
    encrypted_secret TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
