//! File-backed fixture isolation and explicit cleanup. Never used by production storage.
use sqlx::SqlitePool;
use std::{
    io,
    path::Path,
    time::{Duration, Instant},
};

pub async fn close_and_remove(pool: SqlitePool, path: &Path) -> io::Result<()> {
    pool.close().await;
    drop(pool);
    remove_closed_database(path).await
}

async fn remove_closed_database(path: &Path) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut delay = Duration::from_millis(10);
    loop {
        match std::fs::remove_file(path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error)
                if cfg!(windows)
                    && matches!(error.raw_os_error(), Some(32) | Some(33))
                    && Instant::now() < deadline =>
            {
                // A worker or external scanner can briefly retain a Windows file handle.
                // Retry only the sharing/lock violations, never arbitrary cleanup failures.
                tokio::time::sleep(delay.min(deadline.saturating_duration_since(Instant::now())))
                    .await;
                delay = (delay * 2).min(Duration::from_millis(100));
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(windows)]
#[tokio::test]
async fn cleanup_waits_for_a_real_windows_delete_lock_to_be_released() {
    use std::{
        os::windows::fs::OpenOptionsExt,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("locked.db");
    let pool = crate::open(&path).await.unwrap();
    pool.close().await;
    drop(pool);
    // Allow concurrent reads/writes, but deliberately omit FILE_SHARE_DELETE.
    let locked = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0b011)
        .open(&path)
        .unwrap();
    let released = Arc::new(AtomicBool::new(false));
    let flag = released.clone();
    let release = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        drop(locked);
        flag.store(true, Ordering::SeqCst);
    });
    remove_closed_database(&path).await.unwrap();
    assert!(released.load(Ordering::SeqCst));
    assert!(!path.exists());
    release.await.unwrap();
}
