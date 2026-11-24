//! Bootstrap metadata stays at the stable OS location, independently of user data.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LogPolicy {
    pub retention_days: u32,
    pub budget_mib: u32,
}
impl Default for LogPolicy {
    fn default() -> Self {
        Self {
            retention_days: 14,
            budget_mib: 100,
        }
    }
}
impl LogPolicy {
    pub fn validate(&self) -> io::Result<()> {
        if !(1..=90).contains(&self.retention_days) || !(10..=1024).contains(&self.budget_mib) {
            return Err(io::Error::other("日志策略范围：1–90天、10–1024 MiB"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    pub data_dir: Option<PathBuf>,
    pub pending_data_dir: Option<PathBuf>,
    #[serde(default)]
    pub logs: LogPolicy,
}
impl StorageConfig {
    pub fn load(anchor: &Path) -> io::Result<Self> {
        let config = match fs::read(anchor.join("storage.json")) {
            Ok(bytes) => serde_json::from_slice::<Self>(&bytes).map_err(io::Error::other)?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => Self::default(),
            Err(e) => return Err(e),
        };
        config.logs.validate()?;
        if config.data_dir.as_ref().is_some_and(|p| !p.is_absolute())
            || config
                .pending_data_dir
                .as_ref()
                .is_some_and(|p| !p.is_absolute())
        {
            return Err(io::Error::other("数据目录配置必须为绝对路径"));
        }
        Ok(config)
    }
    pub fn active(&self, anchor: &Path) -> PathBuf {
        self.data_dir
            .clone()
            .unwrap_or_else(|| anchor.to_path_buf())
    }
    pub fn save(&self, anchor: &Path) -> io::Result<()> {
        atomic_json(&anchor.join("storage.json"), self)
    }
}
pub fn private_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
pub fn atomic_json(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing parent"))?;
    private_dir(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut file, value).map_err(io::Error::other)?;
    file.flush()?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    #[cfg(unix)]
    {
        fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}
/// Validate every existing path component before touching user-selected storage.
pub fn validate_data_root(path: &Path) -> io::Result<()> {
    if !path.is_absolute() || !path.is_dir() {
        return Err(io::Error::other("数据目录不可用"));
    }
    for ancestor in path.ancestors() {
        if fs::symlink_metadata(ancestor)?.file_type().is_symlink()
            || ancestor.join(".git").exists()
            || ancestor
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("app"))
        {
            return Err(io::Error::other(
                "数据目录不能位于符号链接、Git工作区或.app内",
            ));
        }
    }
    Ok(())
}
/// Resolve only existing, empty directories: no implicit writes to unavailable volumes.
pub fn migration_target(path: &Path, source: &Path, anchor: &Path) -> io::Result<PathBuf> {
    if !path.is_absolute() || !path.is_dir() {
        return Err(io::Error::other("请选择已存在的空目录（绝对路径）"));
    }
    validate_data_root(path)?;
    let target = path.canonicalize()?;
    let source = source.canonicalize()?;
    let anchor = anchor.canonicalize()?;
    if target.starts_with(&source)
        || source.starts_with(&target)
        || target.starts_with(&anchor)
        || anchor.starts_with(&target)
    {
        return Err(io::Error::other("目标目录不能与当前或引导目录重叠"));
    }
    if fs::read_dir(&target)?.next().is_some() {
        return Err(io::Error::other("目标目录必须为空，不能覆盖已有文件"));
    }
    // Check write access without leaving a file behind.
    tempfile::NamedTempFile::new_in(&target)?;
    tempfile::tempdir_in(
        target
            .parent()
            .ok_or_else(|| io::Error::other("目标目录没有父目录"))?,
    )?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_roundtrip_and_reject_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = StorageConfig::default();
        cfg.save(dir.path()).unwrap();
        assert_eq!(StorageConfig::load(dir.path()).unwrap().logs, cfg.logs);
        fs::write(dir.path().join("storage.json"), "broken").unwrap();
        assert!(StorageConfig::load(dir.path()).is_err());
        assert!(LogPolicy {
            retention_days: 0,
            budget_mib: 100
        }
        .validate()
        .is_err());
    }
    #[test]
    fn target_rejects_overlap_and_existing_data() {
        let root = tempfile::tempdir().unwrap();
        let a = root.path().canonicalize().unwrap().join("anchor");
        let b = root.path().canonicalize().unwrap().join("target");
        private_dir(&a).unwrap();
        private_dir(&b).unwrap();
        assert_eq!(
            migration_target(&b, &a, &a).unwrap(),
            b.canonicalize().unwrap()
        );
        assert!(migration_target(&a, &a, &a).is_err());
        private_dir(&a.join("child")).unwrap();
        assert!(migration_target(&a.join("child"), &a, &a).is_err());
        fs::write(b.join("keep"), "x").unwrap();
        assert!(migration_target(&b, &a, &a).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn target_rejects_symlink() {
        let root = tempfile::tempdir().unwrap();
        let a = root.path().join("a");
        let b = root.path().join("b");
        private_dir(&a).unwrap();
        private_dir(&b).unwrap();
        std::os::unix::fs::symlink(&b, root.path().join("link")).unwrap();
        assert!(migration_target(&root.path().join("link"), &a, &a).is_err());
    }
}
