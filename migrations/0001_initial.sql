-- Phase 0 metadata only. No credentials or provider payloads are persisted.
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
