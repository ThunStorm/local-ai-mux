#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("credential store unavailable")]
    Unavailable,
}

/// Opaque references are safe in SQLite; values belong only in the OS store.
pub trait SecretStore: Send + Sync {
    fn put(&self, reference: &str, value: &str) -> Result<(), SecretError>;
    fn get(&self, reference: &str) -> Result<Option<String>, SecretError>;
    fn delete(&self, reference: &str) -> Result<(), SecretError>;
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
        if reference.is_empty()
            || reference.len() > 128
            || !reference
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
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
}
