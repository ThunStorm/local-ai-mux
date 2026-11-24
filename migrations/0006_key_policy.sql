ALTER TABLE api_keys ADD COLUMN allowed_routes TEXT NOT NULL DEFAULT '[]';
ALTER TABLE api_keys ADD COLUMN allowed_connections TEXT NOT NULL DEFAULT '[]';
ALTER TABLE api_keys ADD COLUMN execution_profile TEXT NOT NULL DEFAULT 'INFERENCE_ONLY';
ALTER TABLE api_keys ADD COLUMN rate_limit_per_minute INTEGER NOT NULL DEFAULT 60;
CREATE TABLE credential_refs (
    id TEXT PRIMARY KEY NOT NULL,
    provider_id TEXT NOT NULL,
    credential_kind TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
