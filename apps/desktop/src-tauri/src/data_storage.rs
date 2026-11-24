use localmux_core::data_paths::{
    atomic_json, migration_target, private_dir, validate_data_root, LogPolicy, StorageConfig,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::Manager;

pub struct DataStorage {
    pub anchor: PathBuf,
    pub active: PathBuf,
    pub policy: LogPolicy,
    pub config: Mutex<StorageConfig>,
    _lock: fs::File,
}
fn err(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}
fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn file_digest(path: &Path, hash: &mut Sha256) -> io::Result<()> {
    if !path.exists() {
        hash.update(b"missing");
        return Ok(());
    }
    let mut f = fs::File::open(path)?;
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(())
}
fn fingerprint(source: &Path) -> io::Result<String> {
    let mut h = Sha256::new();
    file_digest(&source.join("localmux.db"), &mut h)?;
    h.update(b"WAL");
    file_digest(&source.join("localmux.db-wal"), &mut h)?;
    Ok(format!("{:x}", h.finalize()))
}
#[derive(Serialize, Deserialize)]
struct Prepared {
    source: PathBuf,
    fingerprint: String,
    target_fingerprint: String,
}
async fn validate_db(path: &Path) -> io::Result<()> {
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(path)
            .read_only(true),
    )
    .await
    .map_err(err)?;
    let result = async {
        let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_one(&pool)
            .await
            .map_err(err)?;
        let foreign = sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .map_err(err)?;
        if integrity != "ok" || !foreign.is_empty() {
            return Err(io::Error::other("数据库完整性校验失败"));
        }
        Ok(())
    }
    .await;
    pool.close().await;
    result
}
/// Page-level snapshot preserves implicit rowids used by legacy response ordering.
async fn database_snapshot(source: &Path, target: &Path) -> io::Result<()> {
    use sqlx::Connection;
    let mut from = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(source)
            .read_only(true),
    )
    .await
    .map_err(err)?;
    let mut to = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(target)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Delete),
    )
    .await
    .map_err(err)?;
    let copied = async {
        let mut source_handle = from.lock_handle().await.map_err(err)?;
        let mut target_handle = to.lock_handle().await.map_err(err)?;
        // SAFETY: SQLx guards exclude worker access for both distinct, live handles.
        // The schema names are static NUL-terminated strings. The backup is finished
        // on every path after successful init, before either guard is released.
        unsafe {
            let backup = libsqlite3_sys::sqlite3_backup_init(
                target_handle.as_raw_handle().as_ptr(),
                c"main".as_ptr(),
                source_handle.as_raw_handle().as_ptr(),
                c"main".as_ptr(),
            );
            if backup.is_null() {
                return Err(err("数据库快照初始化失败"));
            }
            let step = libsqlite3_sys::sqlite3_backup_step(backup, -1);
            let finished = libsqlite3_sys::sqlite3_backup_finish(backup);
            if step != libsqlite3_sys::SQLITE_DONE || finished != libsqlite3_sys::SQLITE_OK {
                return Err(err("数据库快照未完成，未切换路径"));
            }
        }
        Ok(())
    }
    .await;
    let source_closed = from.close().await.map_err(err);
    let target_closed = to.close().await.map_err(err);
    copied?;
    source_closed?;
    target_closed?;
    fs::File::open(target)?.sync_all()?;
    Ok(())
}
fn copy_tree(source: &Path, target: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(source)?;
    if meta.is_symlink() {
        return Err(io::Error::other("迁移拒绝跟随符号链接"));
    }
    if meta.is_dir() {
        private_dir(target)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_tree(&entry.path(), &target.join(entry.file_name()))?;
        }
        #[cfg(unix)]
        fs::File::open(target)?.sync_all()?;
    } else if meta.is_file() {
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            output.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        io::copy(&mut fs::File::open(source)?, &mut output)?;
        output.flush()?;
        output.set_times(fs::FileTimes::new().set_modified(meta.modified()?))?;
        output.sync_all()?;
    } else {
        return Err(io::Error::other("迁移遇到非普通文件"));
    }
    Ok(())
}
/// No source writer is started until publication of config succeeds. Source remains a rollback copy.
async fn migrate(anchor: &Path, config: &mut StorageConfig) -> io::Result<()> {
    let Some(target) = config.pending_data_dir.clone() else {
        return Ok(());
    };
    let source = config.active(anchor).canonicalize()?;
    validate_data_root(&target)?;
    let marker = target.join(".localmux-prepared.json");
    if marker.exists() {
        let prepared: Prepared = serde_json::from_slice(&fs::read(&marker)?).map_err(err)?;
        if prepared.source != source
            || prepared.fingerprint != fingerprint(&source)?
            || prepared.target_fingerprint != fingerprint(&target)?
        {
            return Err(io::Error::other(
                "待恢复迁移与源数据不匹配，请保留两份数据并检查",
            ));
        }
        validate_db(&target.join("localmux.db")).await?;
    } else {
        let target = migration_target(&target, &source, anchor)?;
        validate_db(&source.join("localmux.db")).await?;
        let original = fingerprint(&source)?;
        let stage = tempfile::Builder::new()
            .prefix(".localmux-migration-")
            .tempdir_in(target.parent().ok_or_else(|| err("目标没有父目录"))?)?;
        private_dir(stage.path())?;
        database_snapshot(
            &source.join("localmux.db"),
            &stage.path().join("localmux.db"),
        )
        .await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                stage.path().join("localmux.db"),
                fs::Permissions::from_mode(0o600),
            )?;
        }
        validate_db(&stage.path().join("localmux.db")).await?;
        for name in ["backups", "logs", "storage-samples.json"] {
            let from = source.join(name);
            if from.exists() {
                copy_tree(&from, &stage.path().join(name))?;
            }
        }
        if original != fingerprint(&source)? {
            return Err(err("迁移期间源数据库发生变化，未切换路径"));
        }
        atomic_json(
            &stage.path().join(".localmux-prepared.json"),
            &Prepared {
                source: source.clone(),
                fingerprint: original,
                target_fingerprint: fingerprint(stage.path())?,
            },
        )?;
        // Recheck emptiness just before publication. rename never overwrites user files.
        if fs::read_dir(&target)?.next().is_some() {
            return Err(err("目标目录不再为空"));
        }
        fs::remove_dir(&target)?;
        if let Err(e) = fs::rename(stage.path(), &target) {
            let _ = fs::create_dir(&target);
            return Err(e);
        }
        #[cfg(unix)]
        fs::File::open(target.parent().unwrap())?.sync_all()?;
    }
    let mut next = config.clone();
    next.data_dir = Some(target);
    next.pending_data_dir = None;
    next.save(anchor)?;
    *config = next;
    // A failed config save leaves the ready marker for verified recovery at next startup.
    let _ = fs::remove_file(marker);
    Ok(())
}
impl DataStorage {
    pub async fn boot(anchor: PathBuf) -> io::Result<Self> {
        private_dir(&anchor)?;
        let lock_path = anchor.join("desktop.lock");
        if fs::symlink_metadata(&lock_path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(err("进程锁不能为符号链接"));
        }
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(lock_path)?;
        lock.try_lock()
            .map_err(|_| err("LocalAiMux已运行，请先正常退出再启动"))?;
        let mut config = StorageConfig::load(&anchor)?;
        let current = config.active(&anchor);
        if config.data_dir.is_some() && !current.join("localmux.db").is_file() {
            return Err(err(
                "自定义数据目录不可用，拒绝新建空白历史；请恢复磁盘或storage.json",
            ));
        }
        if config.data_dir.is_some() {
            validate_data_root(&current)?;
        }
        migrate(&anchor, &mut config).await?;
        let active = config.active(&anchor);
        private_dir(&active)?;
        Ok(Self {
            anchor,
            active,
            policy: config.logs.clone(),
            config: Mutex::new(config),
            _lock: lock,
        })
    }
}
#[derive(Serialize)]
pub struct Usage {
    category: &'static str,
    path: String,
    bytes: Option<u64>,
}
fn size(path: &Path) -> io::Result<u64> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    if meta.is_symlink() {
        return Err(err("不统计链接目标"));
    }
    if meta.is_file() {
        return Ok(meta.len());
    }
    let mut total = 0;
    for e in fs::read_dir(path)? {
        total += size(&e?.path())?;
    }
    Ok(total)
}
#[derive(Clone, Serialize, Deserialize)]
struct Sample {
    at: u64,
    bytes: u64,
}
#[derive(Serialize)]
pub struct Growth {
    elapsed_seconds: u64,
    delta_bytes: i64,
    bytes_per_day: f64,
}
fn sample_growth(active: &Path, bytes: u64, now: u64) -> io::Result<Option<Growth>> {
    let path = active.join("storage-samples.json");
    let mut samples: Vec<Sample> = match fs::read(&path) {
        Ok(b) => serde_json::from_slice(&b).map_err(err)?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    samples.retain(|s| s.at <= now && now - s.at <= 30 * 86400);
    if samples.last().is_none_or(|s| now - s.at >= 3600) {
        samples.push(Sample { at: now, bytes });
        atomic_json(&path, &samples)?;
    }
    Ok(samples
        .first()
        .filter(|s| now.saturating_sub(s.at) >= 3600)
        .map(|s| {
            let elapsed = now - s.at;
            let delta = bytes as i64 - s.bytes as i64;
            Growth {
                elapsed_seconds: elapsed,
                delta_bytes: delta,
                bytes_per_day: delta as f64 * 86400.0 / elapsed as f64,
            }
        }))
}
#[derive(Serialize)]
pub struct StorageStatus {
    active_dir: String,
    anchor_dir: String,
    config: StorageConfig,
    active_policy: LogPolicy,
    usage: Vec<Usage>,
    growth: Option<Growth>,
    sampling_ok: bool,
    logs: localmux_telemetry::file_log::LogStatus,
}
fn status(storage: &DataStorage) -> Result<StorageStatus, String> {
    let mut config = storage.config.lock().map_err(|_| "配置锁不可用")?;
    *config = StorageConfig::load(&storage.anchor).map_err(|_| "配置读取失败")?;
    let a = &storage.active;
    let mut usage = Vec::new();
    let db_paths = [
        a.join("localmux.db"),
        a.join("localmux.db-wal"),
        a.join("localmux.db-shm"),
    ];
    let db_size = db_paths
        .iter()
        .try_fold(0, |sum, p| size(p).map(|n| sum + n))
        .ok();
    usage.push(Usage {
        category: "database",
        path: a.join("localmux.db").display().to_string(),
        bytes: db_size,
    });
    for (category, path) in [
        ("backups", a.join("backups")),
        ("logs", a.join("logs")),
        ("local_keys", storage.anchor.join("local-api-keys")),
    ] {
        usage.push(Usage {
            category,
            path: path.display().to_string(),
            bytes: size(&path).ok(),
        });
    }
    let growth = db_size.map(|b| sample_growth(a, b, timestamp()));
    let sampling_ok = matches!(&growth, Some(Ok(_)));
    Ok(StorageStatus {
        active_dir: a.display().to_string(),
        anchor_dir: storage.anchor.display().to_string(),
        config: config.clone(),
        active_policy: storage.policy.clone(),
        usage,
        growth: growth.and_then(Result::ok).flatten(),
        sampling_ok,
        logs: localmux_telemetry::file_log::status(),
    })
}
pub fn start_sampling(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let handle = app.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || {
                status(&handle.state::<DataStorage>())
            })
            .await;
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        }
    });
}
#[tauri::command]
pub async fn data_storage_status(app: tauri::AppHandle) -> Result<StorageStatus, String> {
    tauri::async_runtime::spawn_blocking(move || status(&app.state::<DataStorage>()))
        .await
        .map_err(|_| "占用统计失败")?
}
#[tauri::command]
pub async fn data_storage_save(
    app: tauri::AppHandle,
    path: String,
    retention_days: u32,
    budget_mib: u32,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let storage = app.state::<DataStorage>();
        let logs = LogPolicy {
            retention_days,
            budget_mib,
        };
        logs.validate()
            .map_err(|_| "日志策略范围：1–90天、10–1024 MiB")?;
        let path = PathBuf::from(path.trim());
        let pending = if path == storage.active {
            None
        } else {
            Some(
                migration_target(&path, &storage.active, &storage.anchor).map_err(|_| {
                    "目标须为已存在、可写的空目录；不能为链接、.app内或与当前/引导目录重叠"
                })?,
            )
        };
        let mut config = storage.config.lock().map_err(|_| "配置锁不可用")?;
        *config = StorageConfig::load(&storage.anchor).map_err(|_| "配置读取失败")?;
        let next = StorageConfig {
            pending_data_dir: pending,
            logs,
            ..config.clone()
        };
        next.save(&storage.anchor)
            .map_err(|_| "配置保存失败，请重新读取确认")?;
        *config = next;
        Ok(())
    })
    .await
    .map_err(|_| "配置保存失败")?
}
#[tauri::command]
pub async fn data_storage_pick(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .blocking_pick_folder()
            .map(|p| {
                p.into_path()
                    .map(|p| p.display().to_string())
                    .map_err(|_| "无效目录".to_string())
            })
            .transpose()
    })
    .await
    .map_err(|_| "目录选择失败")?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn cold_migration_preserves_history_keys_and_recovers_config_failure() {
        let dir = tempfile::tempdir().unwrap();
        let anchor = dir.path().canonicalize().unwrap().join("anchor");
        let target = dir.path().canonicalize().unwrap().join("target");
        private_dir(&anchor).unwrap();
        private_dir(&target).unwrap();
        let pool = localmux_storage::open(&anchor.join("localmux.db"))
            .await
            .unwrap();
        localmux_storage::save_setting_bool(&pool, "close_to_tray", true)
            .await
            .unwrap();
        sqlx::query("PRAGMA journal_mode=WAL")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO call_records (id, source, endpoint, requested_model, streamed, started_at, status) VALUES ('recorded', 'api', 'chat', 'fixture/model', 0, 123, 'completed')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO call_contents (call_id, recording_generation, request_json, response_text, state) VALUES ('recorded', 1, '[\"输入\"]', '回答', 'complete')").execute(&pool).await.unwrap();
        localmux_storage::save_response_mapping(
            &pool,
            localmux_storage::NewResponseMapping {
                owner_key_id: "key",
                conversation_id: "conversation",
                response_id: "response",
                model_id: "fixture/model",
                adapter_id: "fixture",
                provider_session_id: Some("native"),
                previous_response_id: None,
                created_at: 123,
            },
        )
        .await
        .unwrap();
        localmux_storage::save_response_mapping(
            &pool,
            localmux_storage::NewResponseMapping {
                owner_key_id: "key",
                conversation_id: "conversation",
                response_id: "latest",
                model_id: "fixture/model",
                adapter_id: "fixture",
                provider_session_id: Some("native"),
                previous_response_id: Some("response"),
                created_at: 123,
            },
        )
        .await
        .unwrap();
        sqlx::query("UPDATE responses SET rowid = rowid + 100")
            .execute(&pool)
            .await
            .unwrap();
        assert!(anchor.join("localmux.db-wal").exists());
        private_dir(&anchor.join("local-api-keys")).unwrap();
        fs::write(anchor.join("local-api-keys/keep"), b"key").unwrap();
        private_dir(&anchor.join("backups")).unwrap();
        fs::write(anchor.join("backups/keep.db"), b"backup").unwrap();
        let mut cfg = StorageConfig {
            pending_data_dir: Some(target.clone()),
            ..Default::default()
        };
        cfg.save(&anchor).unwrap();
        // Force publication to fail after a complete target is prepared.
        fs::remove_file(anchor.join("storage.json")).unwrap();
        fs::create_dir(anchor.join("storage.json")).unwrap();
        assert!(migrate(&anchor, &mut cfg).await.is_err());
        assert!(target.join(".localmux-prepared.json").exists());
        fs::remove_dir(anchor.join("storage.json")).unwrap();
        cfg.save(&anchor).unwrap();
        migrate(&anchor, &mut cfg).await.unwrap();
        assert_eq!(cfg.data_dir, Some(target.clone()));
        assert!(cfg.pending_data_dir.is_none());
        assert!(anchor.join("localmux.db").exists());
        assert!(anchor.join("local-api-keys/keep").exists());
        assert!(!target.join("local-api-keys").exists());
        assert_eq!(fs::read(target.join("backups/keep.db")).unwrap(), b"backup");
        pool.close().await;
        let pool = localmux_storage::open(&target.join("localmux.db"))
            .await
            .unwrap();
        assert_eq!(
            localmux_storage::setting_bool(&pool, "close_to_tray")
                .await
                .unwrap(),
            Some(true)
        );
        let content: String =
            sqlx::query_scalar("SELECT response_text FROM call_contents WHERE call_id='recorded'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(content, "回答");
        assert!(
            localmux_storage::response_conversation(&pool, "latest", "key")
                .await
                .unwrap()
                .unwrap()
                .resume_supported
        );
        let rowid: i64 = sqlx::query_scalar("SELECT rowid FROM responses WHERE id='latest'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rowid, 102);
        assert!(
            localmux_storage::response_conversation(&pool, "response", "key")
                .await
                .unwrap()
                .is_none()
        );
        pool.close().await;
    }
    #[tokio::test]
    async fn missing_custom_db_and_second_instance_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let anchor = dir.path().canonicalize().unwrap().join("anchor");
        let running = DataStorage::boot(anchor.clone()).await.unwrap();
        assert!(DataStorage::boot(anchor.clone()).await.is_err());
        drop(running);
        StorageConfig {
            data_dir: Some(dir.path().join("missing")),
            ..Default::default()
        }
        .save(&anchor)
        .unwrap();
        assert!(DataStorage::boot(anchor).await.is_err());
    }
    #[test]
    fn growth_is_unknown_until_one_hour_and_keeps_negative_net_change() {
        let dir = tempfile::tempdir().unwrap();
        assert!(sample_growth(dir.path(), 1000, 100).unwrap().is_none());
        assert!(sample_growth(dir.path(), 2000, 200).unwrap().is_none());
        let g = sample_growth(dir.path(), 500, 3700).unwrap().unwrap();
        assert_eq!(g.delta_bytes, -500);
        assert_eq!(g.elapsed_seconds, 3600);
    }
}
