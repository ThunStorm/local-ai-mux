//! Development-only local API credentials. Do not use for third-party provider secrets.
use crate::{valid_reference, SecretError, SecretStore};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
};

pub struct LocalFileSecretStore {
    root: PathBuf,
}
impl LocalFileSecretStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
    fn check_directory(&self) -> Result<(), SecretError> {
        if !self.root.exists() {
            match fs::DirBuilder::new().mode(0o700).create(&self.root) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err(SecretError::Unavailable),
            }
        }
        let meta = fs::symlink_metadata(&self.root).map_err(|_| SecretError::Unavailable)?;
        if !meta.is_dir() || meta.permissions().mode() & 0o077 != 0 {
            return Err(SecretError::Unavailable);
        }
        Ok(())
    }
    fn path(&self, reference: &str) -> Result<PathBuf, SecretError> {
        if !valid_reference(reference) || !reference.starts_with("api.key.") {
            return Err(SecretError::Unavailable);
        }
        self.check_directory()?;
        Ok(self.root.join(reference))
    }
    fn check_file(path: &Path) -> Result<bool, SecretError> {
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.is_file() && meta.permissions().mode() & 0o077 == 0 => Ok(true),
            Ok(_) => Err(SecretError::Unavailable),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(_) => Err(SecretError::Unavailable),
        }
    }
}
impl SecretStore for LocalFileSecretStore {
    fn put(&self, reference: &str, value: &str) -> Result<(), SecretError> {
        if value.is_empty() || value.len() > 4096 {
            return Err(SecretError::Unavailable);
        }
        let path = self.path(reference)?;
        Self::check_file(&path)?;
        let mut file =
            tempfile::NamedTempFile::new_in(&self.root).map_err(|_| SecretError::Unavailable)?;
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| SecretError::Unavailable)?;
        file.write_all(value.as_bytes())
            .map_err(|_| SecretError::Unavailable)?;
        file.as_file()
            .sync_all()
            .map_err(|_| SecretError::Unavailable)?;
        file.persist(path).map_err(|_| SecretError::Unavailable)?;
        Ok(())
    }
    fn get(&self, reference: &str) -> Result<Option<String>, SecretError> {
        let path = self.path(reference)?;
        if !Self::check_file(&path)? {
            return Ok(None);
        }
        let mut value = String::new();
        fs::File::open(path)
            .map_err(|_| SecretError::Unavailable)?
            .take(4097)
            .read_to_string(&mut value)
            .map_err(|_| SecretError::Unavailable)?;
        if value.is_empty() || value.len() > 4096 {
            return Err(SecretError::Unavailable);
        }
        Ok(Some(value))
    }
    fn delete(&self, reference: &str) -> Result<(), SecretError> {
        let path = self.path(reference)?;
        if Self::check_file(&path)? {
            fs::remove_file(path).map_err(|_| SecretError::Unavailable)?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credentials_survive_reopen_with_private_permissions_and_atomic_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("keys");
        let store = LocalFileSecretStore::new(root.clone());
        store.put("api.key.fixture", "synthetic-first").unwrap();
        drop(store);
        let store = LocalFileSecretStore::new(root.clone());
        assert_eq!(
            store.get("api.key.fixture").unwrap().as_deref(),
            Some("synthetic-first")
        );
        store.put("api.key.fixture", "synthetic-second").unwrap();
        assert_eq!(
            store.get("api.key.fixture").unwrap().as_deref(),
            Some("synthetic-second")
        );
        assert_eq!(
            fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(root.join("api.key.fixture"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        store.delete("api.key.fixture").unwrap();
        assert!(store.get("api.key.fixture").unwrap().is_none());
    }
    #[test]
    fn unsafe_references_links_permissions_and_oversized_files_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("keys");
        let store = LocalFileSecretStore::new(root.clone());
        for reference in ["../outside", "provider.mimo", "", "api.key./outside"] {
            assert!(store.put(reference, "synthetic").is_err());
        }
        assert!(store.get("api.key.missing").unwrap().is_none());
        let file = root.join("api.key.fixture");
        std::os::unix::fs::symlink(dir.path().join("outside"), &file).unwrap();
        assert!(store.get("api.key.fixture").is_err());
        assert!(store.put("api.key.fixture", "synthetic").is_err());
        fs::remove_file(&file).unwrap();
        store.put("api.key.fixture", "synthetic").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(store.get("api.key.fixture").is_err());
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&file, "x".repeat(4097)).unwrap();
        assert!(store.get("api.key.fixture").is_err());
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(store.get("api.key.fixture").is_err());
    }
}
