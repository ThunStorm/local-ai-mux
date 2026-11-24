CREATE TABLE conversations (
    id TEXT PRIMARY KEY NOT NULL,
    model_id TEXT NOT NULL,
    adapter_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    last_active_at INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'ACTIVE'
);
CREATE TABLE provider_sessions (
    conversation_id TEXT PRIMARY KEY NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    provider_session_id TEXT,
    resume_supported INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE responses (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    model_id TEXT NOT NULL,
    previous_response_id TEXT,
    created_at INTEGER NOT NULL,
    status TEXT NOT NULL
);
CREATE INDEX idx_responses_conversation ON responses(conversation_id, created_at);
CREATE TABLE run_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    detail_json TEXT NOT NULL,
    observed_at INTEGER NOT NULL
);
