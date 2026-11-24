CREATE TABLE api_keys (
    id TEXT PRIMARY KEY NOT NULL,
    key_hash TEXT NOT NULL UNIQUE,
    scopes TEXT NOT NULL,
    revoked_at TEXT
);
