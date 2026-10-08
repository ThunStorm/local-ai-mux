#[cfg(unix)]
mod local_file;
#[cfg(unix)]
pub use local_file::LocalFileSecretStore;
fn valid_reference(reference: &str) -> bool {
    !reference.is_empty()
        && reference.len() <= 128
        && reference
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}
#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("credential store unavailable")]
    Unavailable,
}

/// SQLite contains references/hashes only; each store defines its secret persistence boundary.
pub trait SecretStore: Send + Sync {
    fn put(&self, reference: &str, value: &str) -> Result<(), SecretError>;
    fn get(&self, reference: &str) -> Result<Option<String>, SecretError>;
    fn delete(&self, reference: &str) -> Result<(), SecretError>;
}

/// Development-only secrets: discarded when the desktop process exits.
#[derive(Default)]
pub struct MemorySecretStore {
    values: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

impl SecretStore for MemorySecretStore {
    fn put(&self, reference: &str, value: &str) -> Result<(), SecretError> {
        self.values
            .lock()
            .map_err(|_| SecretError::Unavailable)?
            .insert(reference.to_owned(), value.to_owned());
        Ok(())
    }

    fn get(&self, reference: &str) -> Result<Option<String>, SecretError> {
        Ok(self
            .values
            .lock()
            .map_err(|_| SecretError::Unavailable)?
            .get(reference)
            .cloned())
    }

    fn delete(&self, reference: &str) -> Result<(), SecretError> {
        self.values
            .lock()
            .map_err(|_| SecretError::Unavailable)?
            .remove(reference);
        Ok(())
    }
}

pub struct OsSecretStore {
    service: String,
}
impl OsSecretStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }
    fn entry(&self, reference: &str) -> Result<keyring::Entry, SecretError> {
        if !valid_reference(reference) {
            return Err(SecretError::Unavailable);
        }
        keyring::Entry::new(&self.service, reference).map_err(|_| SecretError::Unavailable)
    }
}
impl SecretStore for OsSecretStore {
    fn put(&self, reference: &str, value: &str) -> Result<(), SecretError> {
        self.entry(reference)?
            .set_password(value)
            .map_err(|_| SecretError::Unavailable)
    }
    fn get(&self, reference: &str) -> Result<Option<String>, SecretError> {
        match self.entry(reference)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(SecretError::Unavailable),
        }
    }
    fn delete(&self, reference: &str) -> Result<(), SecretError> {
        match self.entry(reference)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(SecretError::Unavailable),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_untrusted_credential_reference_before_os_access() {
        let store = OsSecretStore::new("app.localmux.desktop");
        assert!(store.entry("../other-app").is_err());
        assert!(store.entry("").is_err());
    }

    #[test]
    fn memory_secrets_disappear_with_store() {
        let store = MemorySecretStore::default();
        store.put("api.key.test", "secret").unwrap();
        assert_eq!(
            store.get("api.key.test").unwrap().as_deref(),
            Some("secret")
        );
        drop(store);
        assert!(MemorySecretStore::default()
            .get("api.key.test")
            .unwrap()
            .is_none());
    }
}
