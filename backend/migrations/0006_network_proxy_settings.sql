CREATE TABLE network_proxy_settings (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    is_enabled INTEGER NOT NULL DEFAULT 0 CHECK (is_enabled IN (0, 1)),
    protocol TEXT NOT NULL DEFAULT 'http' CHECK (protocol IN ('http', 'https', 'socks5')),
    host TEXT NOT NULL DEFAULT '127.0.0.1',
    port INTEGER NOT NULL DEFAULT 7890 CHECK (port BETWEEN 1 AND 65535),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT INTO network_proxy_settings (id) VALUES (1);
