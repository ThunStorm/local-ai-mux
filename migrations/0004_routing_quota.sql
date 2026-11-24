CREATE TABLE quota_snapshots (
    connection_id TEXT PRIMARY KEY NOT NULL,
    remaining REAL,
    quota_limit REAL,
    unit TEXT,
    reset_at INTEGER,
    source TEXT NOT NULL,
    observed_at INTEGER NOT NULL
);
CREATE TABLE routes (
    id TEXT PRIMARY KEY NOT NULL,
    policy_json TEXT NOT NULL
);
