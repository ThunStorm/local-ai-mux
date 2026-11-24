CREATE TABLE quota_buckets (
    connection_id TEXT NOT NULL,
    bucket_id TEXT NOT NULL,
    label TEXT NOT NULL,
    remaining REAL,
    quota_limit REAL,
    unit TEXT,
    reset_at INTEGER,
    source TEXT NOT NULL,
    observed_at INTEGER NOT NULL,
    PRIMARY KEY (connection_id, bucket_id)
);
INSERT INTO quota_buckets (connection_id, bucket_id, label, remaining, quota_limit, unit, reset_at, source, observed_at)
SELECT connection_id, 'legacy', '上游额度', remaining, quota_limit, unit, reset_at, source, observed_at
FROM quota_snapshots;
