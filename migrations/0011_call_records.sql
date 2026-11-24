CREATE TABLE call_records (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    owner_key_id TEXT,
    source TEXT NOT NULL CHECK(source IN ('api', 'playground', 'model_test')),
    endpoint TEXT NOT NULL CHECK(endpoint IN ('chat', 'responses', 'model_test')),
    requested_model TEXT NOT NULL,
    resolved_model TEXT,
    adapter_id TEXT,
    response_id TEXT,
    conversation_id TEXT,
    streamed INTEGER NOT NULL,
    started_at INTEGER NOT NULL,
    finished_at INTEGER,
    elapsed_ms INTEGER,
    status TEXT NOT NULL CHECK(status IN ('in_progress','completed','failed','timeout','cancelled','truncated','filtered','interrupted')),
    error_code TEXT,
    finish_reason TEXT,
    actual_model TEXT,
    input_tokens INTEGER,
    output_tokens INTEGER,
    total_tokens INTEGER
);
CREATE INDEX idx_call_records_filters ON call_records(source, status, sequence);
