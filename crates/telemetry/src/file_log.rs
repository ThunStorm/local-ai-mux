use flate2::{write::GzEncoder, Compression};
use localmux_core::data_paths::{private_dir, LogPolicy};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
const FILE_LIMIT: u64 = 5 * 1024 * 1024;
static LOGGER: OnceLock<Arc<Mutex<FileLog>>> = OnceLock::new();
#[derive(Clone, serde::Serialize)]
pub struct LogStatus {
    pub healthy: bool,
}
pub struct FileLog {
    dir: PathBuf,
    policy: LogPolicy,
    day: u64,
    healthy: bool,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn regular(path: &Path) -> io::Result<bool> {
    Ok(fs::symlink_metadata(path)?.file_type().is_file())
}
fn archive_time(name: &str) -> Option<(u64, u32)> {
    let inner = name.strip_prefix("localmux-")?.strip_suffix(".log.gz")?;
    let (seconds, counter) = inner.split_once('-')?;
    Some((seconds.parse().ok()?, counter.parse().ok()?))
}
impl FileLog {
    pub fn new(dir: PathBuf, policy: LogPolicy) -> io::Result<Self> {
        policy.validate()?;
        if fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(io::Error::other("log directory is a link"));
        }
        private_dir(&dir)?;
        let active = dir.join("localmux.log");
        let modified = fs::metadata(&active)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| SystemTime::now());
        let mut log = Self {
            dir,
            policy,
            day: modified
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                / 86400,
            healthy: true,
        };
        log.maintenance(now())?;
        // Prove write access at startup even when no event is emitted.
        log.write(&[], now())?;
        Ok(log)
    }
    fn rotate(&mut self, timestamp: u64) -> io::Result<()> {
        let active = self.dir.join("localmux.log");
        if !active.exists() || fs::metadata(&active)?.len() == 0 {
            self.day = timestamp / 86400;
            return Ok(());
        }
        if !regular(&active)? {
            return Err(io::Error::other("log is not a regular file"));
        }
        let mut input = fs::File::open(&active)?;
        let mut tmp = tempfile::NamedTempFile::new_in(&self.dir)?;
        {
            let mut zip = GzEncoder::new(&mut tmp, Compression::default());
            io::copy(&mut input, &mut zip)?;
            zip.finish()?;
        }
        tmp.as_file().sync_all()?;
        let archive_at = self.day * 86400;
        let mut counter = 0u32;
        let dest = loop {
            let p = self
                .dir
                .join(format!("localmux-{archive_at}-{counter}.log.gz"));
            if !p.exists() {
                break p;
            }
            counter = counter
                .checked_add(1)
                .ok_or_else(|| io::Error::other("archive collision"))?;
        };
        tmp.persist_noclobber(dest).map_err(|e| e.error)?;
        #[cfg(unix)]
        fs::File::open(&self.dir)?.sync_all()?;
        // Archive is durable before truncation; an interruption can duplicate an archive, never lose the active log.
        OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&active)?
            .sync_all()?;
        self.day = timestamp / 86400;
        Ok(())
    }
    pub fn maintenance(&mut self, timestamp: u64) -> io::Result<()> {
        let active = self.dir.join("localmux.log");
        if active.exists() {
            if !regular(&active)? {
                return Err(io::Error::other("log is not a regular file"));
            }
            if self.day != timestamp / 86400 || fs::metadata(&active)?.len() >= FILE_LIMIT {
                self.rotate(timestamp)?;
            }
        }
        let mut archives = Vec::new();
        let mut total = fs::metadata(&active).map(|m| m.len()).unwrap_or(0);
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let Some((t, counter)) = archive_time(&entry.file_name().to_string_lossy()) else {
                continue;
            };
            if !entry.file_type()?.is_file() {
                continue;
            }
            if timestamp.saturating_sub(t) >= u64::from(self.policy.retention_days) * 86400 {
                fs::remove_file(entry.path())?;
            } else {
                let size = entry.metadata()?.len();
                total += size;
                archives.push((t, counter, entry.path(), size));
            }
        }
        archives.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        for (_, _, path, size) in archives {
            // Leave enough headroom for the next event / full active file.
            if total <= u64::from(self.policy.budget_mib) * 1024 * 1024 - FILE_LIMIT {
                break;
            }
            fs::remove_file(path)?;
            total = total.saturating_sub(size);
        }
        Ok(())
    }
    pub fn write(&mut self, bytes: &[u8], timestamp: u64) -> io::Result<()> {
        let bytes = &bytes[..bytes.len().min(64 * 1024)];
        let active = self.dir.join("localmux.log");
        if active.exists() && !regular(&active)? {
            return Err(io::Error::other("log is not a regular file"));
        }
        let length = fs::metadata(&active).map(|m| m.len()).unwrap_or(0);
        if length + bytes.len() as u64 > FILE_LIMIT || self.day != timestamp / 86400 {
            self.rotate(timestamp)?;
            self.maintenance(timestamp)?;
        }
        let mut options = OpenOptions::new();
        options.append(true).create(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(active)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        file.write_all(bytes)?;
        Ok(())
    }
}
pub fn init(dir: PathBuf, policy: LogPolicy) -> io::Result<()> {
    let mut writer = FileLog::new(dir, policy)?;
    // This fixed, secret-free marker proves file availability even when RUST_LOG
    // suppresses info events (for example inherited RUST_LOG=warn from a launcher).
    writer.write(b"LocalAiMux: diagnostic file logging initialized\n", now())?;
    let log = Arc::new(Mutex::new(writer));
    LOGGER
        .set(log.clone())
        .map_err(|_| io::Error::other("logger already configured"))?;
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(60));
        if let Ok(mut log) = log.lock() {
            log.healthy = log.maintenance(now()).is_ok();
        }
    });
    Ok(())
}
pub fn append(bytes: &[u8]) {
    if let Some(log) = LOGGER.get() {
        if let Ok(mut log) = log.lock() {
            log.healthy = log.write(bytes, now()).is_ok();
        }
    }
}
pub fn status() -> LogStatus {
    LogStatus {
        healthy: LOGGER
            .get()
            .and_then(|l| l.lock().ok().map(|l| l.healthy))
            .unwrap_or(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn archives_are_gzip_and_expired_files_only_are_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = FileLog::new(dir.path().to_path_buf(), LogPolicy::default()).unwrap();
        let t = now();
        log.write(b"safe diagnostic\n", t).unwrap();
        log.write(b"new day\n", t + 86400).unwrap();
        let archive = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .find(|e| e.path().extension().is_some_and(|e| e == "gz"))
            .unwrap();
        let mut gz = flate2::read::GzDecoder::new(fs::File::open(archive.path()).unwrap());
        let mut text = String::new();
        use std::io::Read;
        gz.read_to_string(&mut text).unwrap();
        assert_eq!(text, "safe diagnostic\n");
        fs::write(dir.path().join("keep.db"), b"history").unwrap();
        log.maintenance(t + 16 * 86400).unwrap();
        assert!(!archive.path().exists());
        assert!(dir.path().join("keep.db").exists());
    }
    #[test]
    fn budget_and_size_are_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = FileLog::new(
            dir.path().to_path_buf(),
            LogPolicy {
                retention_days: 14,
                budget_mib: 10,
            },
        )
        .unwrap();
        let t = now();
        for n in 0..4 {
            fs::write(
                dir.path().join(format!("localmux-{}-0.log.gz", t - n)),
                vec![0u8; 3 * 1024 * 1024],
            )
            .unwrap();
        }
        log.maintenance(t).unwrap();
        let total: u64 = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().metadata().unwrap().len())
            .sum();
        assert!(total <= 10 * 1024 * 1024);
        fs::write(
            dir.path().join("localmux.log"),
            vec![b'x'; FILE_LIMIT as usize],
        )
        .unwrap();
        log.write(b"next", t).unwrap();
        assert_eq!(
            fs::metadata(dir.path().join("localmux.log")).unwrap().len(),
            4
        );
    }
    #[test]
    fn same_day_budget_evicts_by_numeric_rotation_order() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = FileLog::new(
            dir.path().to_path_buf(),
            LogPolicy {
                retention_days: 14,
                budget_mib: 10,
            },
        )
        .unwrap();
        let t = now();
        for n in 0..14 {
            fs::write(
                dir.path().join(format!("localmux-{t}-{n}.log.gz")),
                vec![0u8; 512 * 1024],
            )
            .unwrap();
        }
        log.maintenance(t).unwrap();
        for n in 0..4 {
            assert!(!dir.path().join(format!("localmux-{t}-{n}.log.gz")).exists());
        }
        for n in 4..14 {
            assert!(dir.path().join(format!("localmux-{t}-{n}.log.gz")).exists());
        }
    }
    #[cfg(unix)]
    #[test]
    fn symlink_is_never_truncated() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("history");
        fs::write(&outside, b"keep").unwrap();
        std::os::unix::fs::symlink(&outside, dir.path().join("localmux.log")).unwrap();
        assert!(FileLog::new(dir.path().to_path_buf(), LogPolicy::default()).is_err());
        assert_eq!(fs::read(outside).unwrap(), b"keep");
    }
}
