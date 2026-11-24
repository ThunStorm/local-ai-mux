-- Explicitly added desktop installations. A row does not create a Provider connection.
CREATE TABLE added_applications (
    install_path TEXT PRIMARY KEY NOT NULL,
    app_id TEXT NOT NULL,
    added_at TEXT NOT NULL DEFAULT (datetime('now'))
);
