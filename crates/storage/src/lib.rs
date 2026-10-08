use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::path::Path;

static MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

/// Open the local database and apply versioned migrations before any UI uses it.
pub async fn open(path: &Path) -> anyhow::Result<SqlitePool> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let options = SqliteConnectOptions::new()
        .filename(path)
        .foreign_keys(true)
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await?;
    MIGRATIONS.run(&pool).await?;
    Ok(pool)
}
pub async fn open_memory() -> anyhow::Result<SqlitePool> {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;
    MIGRATIONS.run(&pool).await?;
    Ok(pool)
}
pub async fn setting_bool(pool: &SqlitePool, key: &str) -> anyhow::Result<Option<bool>> {
    let value = setting_string(pool, key).await?;
    Ok(value.map(|value| value == "true"))
}
pub async fn setting_string(pool: &SqlitePool, key: &str) -> anyhow::Result<Option<String>> {
    Ok(
        sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(pool)
            .await?,
    )
}
pub async fn save_setting_bool(pool: &SqlitePool, key: &str, value: bool) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(key).bind(if value { "true" } else { "false" }).execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn initializes_schema() {
        let path = std::env::temp_dir().join(format!("localmux-phase0-{}.db", std::process::id()));
        let pool = super::open(&path).await.expect("database opens");
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='settings'",
        )
        .fetch_one(&pool)
        .await
        .expect("query schema");
        assert_eq!(count, 1);
        let version: i64 = sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .expect("latest migration");
        assert_eq!(version, 10);
        pool.close().await;
        std::fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn added_application_is_separate_from_connection() {
        let pool = super::open_memory().await.unwrap();
        super::upsert_added_application(&pool, "mimo", "/Applications/Xiaomi MiMo.app")
            .await
            .unwrap();
        super::upsert_added_application(&pool, "mimo", "/Applications/Xiaomi MiMo.app")
            .await
            .unwrap();
        assert_eq!(
            super::saved_added_applications(&pool).await.unwrap(),
            vec![("mimo".into(), "/Applications/Xiaomi MiMo.app".into())]
        );
        assert!(super::saved_connections(&pool).await.unwrap().is_empty());
        super::delete_added_application(&pool, "/Applications/Xiaomi MiMo.app")
            .await
            .unwrap();
        assert!(super::saved_added_applications(&pool)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn default_key_rotation_revokes_old_key_atomically() {
        let pool = super::open_memory().await.unwrap();
        for id in ["old", "new"] {
            super::insert_api_key(&pool, id, &format!("hash-{id}"), &["models:read".into()])
                .await
                .unwrap();
        }
        super::replace_default_api_key(&pool, "old", None)
            .await
            .unwrap();
        super::replace_default_api_key(&pool, "new", Some("old"))
            .await
            .unwrap();
        assert_eq!(
            super::setting_string(&pool, "default_api_key_id")
                .await
                .unwrap()
                .as_deref(),
            Some("new")
        );
        assert!(!super::api_key_active_by_id(&pool, "old").await.unwrap());
        assert!(super::api_key_active_by_id(&pool, "new").await.unwrap());
    }

    #[tokio::test]
    async fn quota_buckets_preserve_independent_windows_on_failed_refresh() {
        let pool = super::open_memory().await.unwrap();
        let first = localmux_quota::QuotaBucket {
            connection_id: "codex".into(),
            bucket_id: "primary".into(),
            label: "300 min window".into(),
            remaining: Some(75.0),
            limit: Some(100.0),
            unit: Some("%".into()),
            reset_at: Some(100),
            source: localmux_quota::QuotaSource::ProviderReported,
            observed_at: 1,
        };
        let second = localmux_quota::QuotaBucket {
            bucket_id: "secondary".into(),
            reset_at: Some(200),
            ..first.clone()
        };
        super::save_quota_buckets(&pool, "codex", &[first.clone(), second])
            .await
            .unwrap();
        let saved = super::load_quota_buckets(&pool).await.unwrap();
        assert_eq!(saved.len(), 2);
        assert_ne!(saved[0].reset_at, saved[1].reset_at);
        assert!(
            super::save_quota_buckets(&pool, "codex", &[first.clone(), first])
                .await
                .is_err()
        );
        assert_eq!(super::load_quota_buckets(&pool).await.unwrap().len(), 2);
    }
}

#[derive(Debug, Clone)]
pub struct ApiKeyRecord {
    pub id: String,
    pub scopes: Vec<String>,
    pub policy: localmux_security::KeyRestrictions,
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct ApiKeySummary {
    pub id: String,
    pub scopes: Vec<String>,
    pub policy: localmux_security::KeyRestrictions,
}
pub async fn list_api_keys(pool: &SqlitePool) -> anyhow::Result<Vec<ApiKeySummary>> {
    use sqlx::Row;
    let rows = sqlx::query("SELECT id, scopes, allowed_routes, allowed_connections, execution_profile, rate_limit_per_minute FROM api_keys WHERE revoked_at IS NULL ORDER BY rowid DESC")
        .fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            let profile: String = row.try_get("execution_profile")?;
            Ok(ApiKeySummary {
                id: row.try_get("id")?,
                scopes: serde_json::from_str(row.try_get::<&str, _>("scopes")?)?,
                policy: localmux_security::KeyRestrictions {
                    allowed_routes: serde_json::from_str(
                        row.try_get::<&str, _>("allowed_routes")?,
                    )?,
                    allowed_connections: serde_json::from_str(
                        row.try_get::<&str, _>("allowed_connections")?,
                    )?,
                    execution_profile: serde_json::from_value(serde_json::Value::String(profile))?,
                    rate_limit_per_minute: row
                        .try_get::<i64, _>("rate_limit_per_minute")?
                        .try_into()?,
                },
            })
        })
        .collect()
}

pub async fn insert_api_key(
    pool: &SqlitePool,
    id: &str,
    hash: &str,
    scopes: &[String],
) -> anyhow::Result<()> {
    insert_api_key_with_policy(
        pool,
        id,
        hash,
        scopes,
        &localmux_security::KeyRestrictions::default(),
    )
    .await
}

pub async fn insert_api_key_with_policy(
    pool: &SqlitePool,
    id: &str,
    hash: &str,
    scopes: &[String],
    policy: &localmux_security::KeyRestrictions,
) -> anyhow::Result<()> {
    let scopes_json = serde_json::to_string(scopes)?;
    let routes_json = serde_json::to_string(&policy.allowed_routes)?;
    let connections_json = serde_json::to_string(&policy.allowed_connections)?;
    let profile_json = serde_json::to_value(policy.execution_profile)?;
    let profile = profile_json
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("invalid execution profile"))?;
    sqlx::query("INSERT INTO api_keys (id, key_hash, scopes, allowed_routes, allowed_connections, execution_profile, rate_limit_per_minute) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(id)
        .bind(hash)
        .bind(scopes_json)
        .bind(routes_json)
        .bind(connections_json)
        .bind(profile)
        .bind(i64::from(policy.rate_limit_per_minute))
        .execute(pool)
        .await?;
    Ok(())
}

/// Explicitly change an active key's access while preserving its ID and hash.
pub async fn update_api_key_access(
    pool: &SqlitePool,
    id: &str,
    scopes: &[String],
    policy: &localmux_security::KeyRestrictions,
) -> anyhow::Result<()> {
    if !policy.valid() {
        anyhow::bail!("invalid API key policy");
    }
    let profile = serde_json::to_value(policy.execution_profile)?;
    let result = sqlx::query("UPDATE api_keys SET scopes = ?, allowed_routes = ?, allowed_connections = ?, execution_profile = ?, rate_limit_per_minute = ? WHERE id = ? AND revoked_at IS NULL")
        .bind(serde_json::to_string(scopes)?).bind(serde_json::to_string(&policy.allowed_routes)?).bind(serde_json::to_string(&policy.allowed_connections)?)
        .bind(profile.as_str().ok_or_else(|| anyhow::anyhow!("invalid execution profile"))?).bind(i64::from(policy.rate_limit_per_minute)).bind(id).execute(pool).await?;
    if result.rows_affected() != 1 {
        anyhow::bail!("active API key not found");
    }
    Ok(())
}

pub async fn active_api_key(pool: &SqlitePool, hash: &str) -> anyhow::Result<Option<ApiKeyRecord>> {
    use sqlx::Row;
    let row =
        sqlx::query("SELECT id, scopes, allowed_routes, allowed_connections, execution_profile, rate_limit_per_minute FROM api_keys WHERE key_hash = ? AND revoked_at IS NULL")
            .bind(hash)
            .fetch_optional(pool)
            .await?;
    row.map(|row| {
        let profile: String = row.try_get("execution_profile")?;
        Ok(ApiKeyRecord {
            id: row.try_get("id")?,
            scopes: serde_json::from_str(row.try_get::<&str, _>("scopes")?)?,
            policy: localmux_security::KeyRestrictions {
                allowed_routes: serde_json::from_str(row.try_get::<&str, _>("allowed_routes")?)?,
                allowed_connections: serde_json::from_str(
                    row.try_get::<&str, _>("allowed_connections")?,
                )?,
                execution_profile: serde_json::from_value(serde_json::Value::String(profile))?,
                rate_limit_per_minute: row
                    .try_get::<i64, _>("rate_limit_per_minute")?
                    .try_into()?,
            },
        })
    })
    .transpose()
}

pub async fn revoke_api_key(pool: &SqlitePool, id: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE api_keys SET revoked_at = datetime('now') WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
pub async fn api_key_active_by_id(pool: &SqlitePool, id: &str) -> anyhow::Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM api_keys WHERE id = ? AND revoked_at IS NULL",
    )
    .bind(id)
    .fetch_one(pool)
    .await?
        == 1)
}

pub async fn save_quota_buckets(
    pool: &SqlitePool,
    connection_id: &str,
    buckets: &[localmux_quota::QuotaBucket],
) -> anyhow::Result<()> {
    if buckets.is_empty()
        || buckets
            .iter()
            .any(|item| item.connection_id != connection_id || item.bucket_id.is_empty())
    {
        anyhow::bail!("invalid quota buckets");
    }
    let mut transaction = pool.begin().await?;
    sqlx::query("DELETE FROM quota_buckets WHERE connection_id = ?")
        .bind(connection_id)
        .execute(&mut *transaction)
        .await?;
    for bucket in buckets {
        let source = serde_json::to_value(bucket.source)?;
        sqlx::query("INSERT INTO quota_buckets (connection_id, bucket_id, label, remaining, quota_limit, unit, reset_at, source, observed_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&bucket.connection_id).bind(&bucket.bucket_id).bind(&bucket.label)
            .bind(bucket.remaining).bind(bucket.limit).bind(&bucket.unit).bind(bucket.reset_at)
            .bind(source.as_str().ok_or_else(|| anyhow::anyhow!("invalid quota source"))?)
            .bind(bucket.observed_at).execute(&mut *transaction).await?;
    }
    transaction.commit().await?;
    Ok(())
}

pub async fn load_quota_buckets(
    pool: &SqlitePool,
) -> anyhow::Result<Vec<localmux_quota::QuotaBucket>> {
    use sqlx::Row;
    let rows = sqlx::query("SELECT connection_id, bucket_id, label, remaining, quota_limit, unit, reset_at, source, observed_at FROM quota_buckets ORDER BY connection_id, bucket_id")
        .fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            Ok(localmux_quota::QuotaBucket {
                connection_id: row.try_get("connection_id")?,
                bucket_id: row.try_get("bucket_id")?,
                label: row.try_get("label")?,
                remaining: row.try_get("remaining")?,
                limit: row.try_get("quota_limit")?,
                unit: row.try_get("unit")?,
                reset_at: row.try_get("reset_at")?,
                source: serde_json::from_value(serde_json::Value::String(row.try_get("source")?))?,
                observed_at: row.try_get("observed_at")?,
            })
        })
        .collect()
}

pub async fn replace_default_api_key(
    pool: &SqlitePool,
    new_id: &str,
    old_id: Option<&str>,
) -> anyhow::Result<()> {
    let mut transaction = pool.begin().await?;
    sqlx::query("INSERT INTO settings (key, value) VALUES ('default_api_key_id', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(new_id).execute(&mut *transaction).await?;
    sqlx::query("INSERT INTO settings (key, value) VALUES ('default_api_key_store_version', '1') ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .execute(&mut *transaction).await?;
    if let Some(old_id) = old_id {
        sqlx::query("UPDATE api_keys SET revoked_at = datetime('now') WHERE id = ?")
            .bind(old_id)
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await?;
    Ok(())
}

pub async fn upsert_connection(
    pool: &SqlitePool,
    id: &str,
    adapter_id: &str,
    executable: &str,
) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO connections (id, adapter_id, executable_path) VALUES (?, ?, ?) ON CONFLICT(id) DO UPDATE SET executable_path = excluded.executable_path")
        .bind(id).bind(adapter_id).bind(executable).execute(pool).await?;
    Ok(())
}
pub async fn connection_executable(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<String>> {
    sqlx::query_scalar("SELECT executable_path FROM connections WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(Into::into)
}
pub async fn delete_connection(pool: &SqlitePool, id: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM connections WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
pub async fn saved_connections(pool: &SqlitePool) -> anyhow::Result<Vec<(String, String)>> {
    use sqlx::Row;
    let rows = sqlx::query("SELECT id, executable_path FROM connections")
        .fetch_all(pool)
        .await?;
    rows.into_iter()
        .map(|row| Ok((row.try_get("id")?, row.try_get("executable_path")?)))
        .collect()
}

pub async fn upsert_added_application(
    pool: &SqlitePool,
    app_id: &str,
    install_path: &str,
) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO added_applications (install_path, app_id) VALUES (?, ?) ON CONFLICT(install_path) DO UPDATE SET app_id = excluded.app_id")
        .bind(install_path).bind(app_id).execute(pool).await?;
    Ok(())
}

pub async fn saved_added_applications(pool: &SqlitePool) -> anyhow::Result<Vec<(String, String)>> {
    sqlx::query_as(
        "SELECT app_id, install_path FROM added_applications ORDER BY app_id, install_path",
    )
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn delete_added_application(pool: &SqlitePool, install_path: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM added_applications WHERE install_path = ?")
        .bind(install_path)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn save_quota_snapshot(
    pool: &SqlitePool,
    snapshot: &localmux_quota::QuotaSnapshot,
) -> anyhow::Result<()> {
    let source = serde_json::to_string(&snapshot.source)?;
    sqlx::query("INSERT INTO quota_snapshots (connection_id, remaining, quota_limit, unit, reset_at, source, observed_at) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(connection_id) DO UPDATE SET remaining=excluded.remaining, quota_limit=excluded.quota_limit, unit=excluded.unit, reset_at=excluded.reset_at, source=excluded.source, observed_at=excluded.observed_at")
        .bind(&snapshot.connection_id).bind(snapshot.remaining).bind(snapshot.limit).bind(&snapshot.unit)
        .bind(snapshot.reset_at).bind(source).bind(snapshot.observed_at).execute(pool).await?;
    Ok(())
}
pub async fn load_quota_snapshots(
    pool: &SqlitePool,
) -> anyhow::Result<Vec<localmux_quota::QuotaSnapshot>> {
    use sqlx::Row;
    let rows = sqlx::query("SELECT connection_id, remaining, quota_limit, unit, reset_at, source, observed_at FROM quota_snapshots").fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            let source: String = row.try_get("source")?;
            Ok(localmux_quota::QuotaSnapshot {
                connection_id: row.try_get("connection_id")?,
                remaining: row.try_get("remaining")?,
                limit: row.try_get("quota_limit")?,
                unit: row.try_get("unit")?,
                reset_at: row.try_get("reset_at")?,
                source: serde_json::from_str(&source)?,
                observed_at: row.try_get("observed_at")?,
            })
        })
        .collect()
}
pub async fn save_route_policy(
    pool: &SqlitePool,
    id: &str,
    policy_json: &str,
) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO routes (id, policy_json) VALUES (?, ?) ON CONFLICT(id) DO UPDATE SET policy_json=excluded.policy_json")
        .bind(id).bind(policy_json).execute(pool).await?;
    Ok(())
}
pub async fn load_route_policy(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<String>> {
    sqlx::query_scalar("SELECT policy_json FROM routes WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(Into::into)
}

#[cfg(test)]
mod routing_storage_tests {
    use super::*;
    #[tokio::test]
    async fn persists_route_policy_and_reported_quota() {
        let path = std::env::temp_dir().join(format!("localmux-phase6-{}.db", std::process::id()));
        let pool = open(&path).await.unwrap();
        save_route_policy(&pool, "default", r#"{"reserve_as_free":false}"#)
            .await
            .unwrap();
        assert_eq!(
            load_route_policy(&pool, "default")
                .await
                .unwrap()
                .as_deref(),
            Some(r#"{"reserve_as_free":false}"#)
        );
        let snapshot = localmux_quota::QuotaSnapshot {
            connection_id: "fixture".into(),
            remaining: Some(3.0),
            limit: Some(5.0),
            unit: Some("credits".into()),
            reset_at: None,
            source: localmux_quota::QuotaSource::ProviderReported,
            observed_at: 123,
        };
        save_quota_snapshot(&pool, &snapshot).await.unwrap();
        let rows = load_quota_snapshots(&pool).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].source,
            localmux_quota::QuotaSource::ProviderReported
        );
        pool.close().await;
        std::fs::remove_file(path).ok();
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ConversationState {
    pub id: String,
    pub model_id: String,
    pub adapter_id: String,
    pub provider_session_id: Option<String>,
    pub resume_supported: bool,
    pub created_at: i64,
    pub last_active_at: i64,
    pub status: String,
}
pub async fn response_conversation(
    pool: &SqlitePool,
    response_id: &str,
    owner_key_id: &str,
) -> anyhow::Result<Option<ConversationState>> {
    use sqlx::Row;
    let row = sqlx::query("SELECT c.id, c.model_id, c.adapter_id, c.created_at, c.last_active_at, c.status, p.provider_session_id, p.resume_supported FROM responses r JOIN conversations c ON c.id = r.conversation_id LEFT JOIN provider_sessions p ON p.conversation_id = c.id WHERE r.id = ? AND c.owner_key_id = ? AND r.status = 'completed' AND r.rowid = (SELECT MAX(rowid) FROM responses WHERE conversation_id = c.id)")
        .bind(response_id).bind(owner_key_id).fetch_optional(pool).await?;
    row.map(|row| {
        Ok(ConversationState {
            id: row.try_get("id")?,
            model_id: row.try_get("model_id")?,
            adapter_id: row.try_get("adapter_id")?,
            provider_session_id: row.try_get("provider_session_id")?,
            resume_supported: row
                .try_get::<Option<i64>, _>("resume_supported")?
                .unwrap_or(0)
                != 0,
            created_at: row.try_get("created_at")?,
            last_active_at: row.try_get("last_active_at")?,
            status: row.try_get("status")?,
        })
    })
    .transpose()
}
pub struct NewResponseMapping<'a> {
    pub owner_key_id: &'a str,
    pub conversation_id: &'a str,
    pub response_id: &'a str,
    pub model_id: &'a str,
    pub adapter_id: &'a str,
    pub provider_session_id: Option<&'a str>,
    pub previous_response_id: Option<&'a str>,
    pub created_at: i64,
}
pub async fn save_response_mapping(
    pool: &SqlitePool,
    mapping: NewResponseMapping<'_>,
) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    let saved = sqlx::query("INSERT INTO conversations (id, model_id, adapter_id, created_at, last_active_at, owner_key_id) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET last_active_at=excluded.last_active_at WHERE conversations.owner_key_id=excluded.owner_key_id")
        .bind(mapping.conversation_id).bind(mapping.model_id).bind(mapping.adapter_id).bind(mapping.created_at).bind(mapping.created_at).bind(mapping.owner_key_id).execute(&mut *tx).await?;
    if saved.rows_affected() != 1 {
        anyhow::bail!("conversation belongs to another API key");
    }
    sqlx::query("INSERT INTO provider_sessions (conversation_id, provider_session_id, resume_supported) VALUES (?, ?, ?) ON CONFLICT(conversation_id) DO UPDATE SET provider_session_id=COALESCE(excluded.provider_session_id, provider_sessions.provider_session_id), resume_supported=MAX(provider_sessions.resume_supported, excluded.resume_supported)")
        .bind(mapping.conversation_id).bind(mapping.provider_session_id).bind(i64::from(mapping.provider_session_id.is_some())).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO responses (id, conversation_id, model_id, previous_response_id, created_at, status) VALUES (?, ?, ?, ?, ?, 'completed')")
        .bind(mapping.response_id).bind(mapping.conversation_id).bind(mapping.model_id).bind(mapping.previous_response_id).bind(mapping.created_at).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
pub async fn list_conversations(pool: &SqlitePool) -> anyhow::Result<Vec<ConversationState>> {
    use sqlx::Row;
    let rows = sqlx::query("SELECT c.id, c.model_id, c.adapter_id, c.created_at, c.last_active_at, c.status, p.provider_session_id, p.resume_supported FROM conversations c LEFT JOIN provider_sessions p ON p.conversation_id = c.id ORDER BY c.last_active_at DESC LIMIT 100").fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            Ok(ConversationState {
                id: row.try_get("id")?,
                model_id: row.try_get("model_id")?,
                adapter_id: row.try_get("adapter_id")?,
                provider_session_id: row.try_get("provider_session_id")?,
                resume_supported: row
                    .try_get::<Option<i64>, _>("resume_supported")?
                    .unwrap_or(0)
                    != 0,
                created_at: row.try_get("created_at")?,
                last_active_at: row.try_get("last_active_at")?,
                status: row.try_get("status")?,
            })
        })
        .collect()
}
pub async fn record_run_event(
    pool: &SqlitePool,
    conversation_id: &str,
    kind: &str,
    detail_json: &str,
    now: i64,
) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO run_events (conversation_id, kind, detail_json, observed_at) VALUES (?, ?, ?, ?)")
        .bind(conversation_id).bind(kind).bind(detail_json).bind(now).execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod session_storage_tests {
    use super::*;
    #[tokio::test]
    async fn maps_latest_response_without_persisting_prompt() {
        let path = std::env::temp_dir().join(format!("localmux-phase7-{}.db", std::process::id()));
        let pool = open(&path).await.unwrap();
        save_response_mapping(
            &pool,
            NewResponseMapping {
                owner_key_id: "key-a",
                conversation_id: "conversation-test",
                response_id: "resp-one",
                model_id: "codex/model",
                adapter_id: "codex",
                provider_session_id: Some("native-id"),
                previous_response_id: None,
                created_at: 100,
            },
        )
        .await
        .unwrap();
        let state = response_conversation(&pool, "resp-one", "key-a")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(state.provider_session_id.as_deref(), Some("native-id"));
        assert!(state.resume_supported);
        assert!(response_conversation(&pool, "resp-one", "key-b")
            .await
            .unwrap()
            .is_none());
        save_response_mapping(
            &pool,
            NewResponseMapping {
                owner_key_id: "key-a",
                conversation_id: "conversation-test",
                response_id: "resp-two",
                model_id: "codex/model",
                adapter_id: "codex",
                provider_session_id: None,
                previous_response_id: Some("resp-one"),
                created_at: 101,
            },
        )
        .await
        .unwrap();
        assert!(response_conversation(&pool, "resp-one", "key-a")
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            response_conversation(&pool, "resp-two", "key-a")
                .await
                .unwrap()
                .unwrap()
                .provider_session_id
                .as_deref(),
            Some("native-id")
        );
        pool.close().await;
        std::fs::remove_file(path).ok();
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct RunEvent {
    pub kind: String,
    pub detail_json: String,
    pub observed_at: i64,
}
pub async fn list_run_events(
    pool: &SqlitePool,
    conversation_id: &str,
) -> anyhow::Result<Vec<RunEvent>> {
    use sqlx::Row;
    let rows = sqlx::query("SELECT kind, detail_json, observed_at FROM run_events WHERE conversation_id = ? ORDER BY id DESC LIMIT 100")
        .bind(conversation_id).fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            Ok(RunEvent {
                kind: row.try_get("kind")?,
                detail_json: row.try_get("detail_json")?,
                observed_at: row.try_get("observed_at")?,
            })
        })
        .collect()
}
