CREATE TABLE connections (
    id TEXT PRIMARY KEY NOT NULL,
    adapter_id TEXT NOT NULL,
    executable_path TEXT NOT NULL,
    connected_at TEXT NOT NULL DEFAULT (datetime('now'))
);
