use std::collections::HashMap;
use std::sync::Mutex;

const SERVICE: &str = "cogit";

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("a secret cannot be empty")]
    Empty,
    #[error("the credential store is unavailable: {0}")]
    Store(String),
}

/// The store is behind a trait so tests never touch the developer's real keychain, and so
/// a platform without one still gets a working application (doc/12-risks.md, R-06).
pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&self, key: &str, secret: &str) -> Result<(), SecretError>;
    fn delete(&self, key: &str) -> Result<(), SecretError>;
}

#[derive(Debug, Default)]
pub struct MemoryStore {
    secrets: Mutex<HashMap<String, String>>,
}

impl SecretStore for MemoryStore {
    fn get(&self, key: &str) -> Option<String> {
        self.secrets.lock().ok()?.get(key).cloned()
    }

    fn set(&self, key: &str, secret: &str) -> Result<(), SecretError> {
        if secret.is_empty() {
            return Err(SecretError::Empty);
        }
        let mut secrets = self
            .secrets
            .lock()
            .map_err(|err| SecretError::Store(err.to_string()))?;
        secrets.insert(key.to_owned(), secret.to_owned());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), SecretError> {
        let mut secrets = self
            .secrets
            .lock()
            .map_err(|err| SecretError::Store(err.to_string()))?;
        secrets.remove(key);
        Ok(())
    }
}

#[derive(Debug)]
pub struct KeyringStore;

impl KeyringStore {
    fn entry(key: &str) -> Result<keyring::Entry, SecretError> {
        keyring::Entry::new(SERVICE, key).map_err(|err| SecretError::Store(err.to_string()))
    }
}

impl SecretStore for KeyringStore {
    fn get(&self, key: &str) -> Option<String> {
        match Self::entry(key).ok()?.get_password() {
            Ok(secret) => Some(secret),
            Err(keyring::Error::NoEntry) => None,
            Err(err) => {
                tracing::error!(error = ?err, context = "failed to read a stored secret");
                None
            }
        }
    }

    fn set(&self, key: &str, secret: &str) -> Result<(), SecretError> {
        if secret.is_empty() {
            return Err(SecretError::Empty);
        }
        Self::entry(key)?
            .set_password(secret)
            .map_err(|err| SecretError::Store(err.to_string()))
    }

    fn delete(&self, key: &str) -> Result<(), SecretError> {
        match Self::entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(SecretError::Store(err.to_string())),
        }
    }
}

/// Falls back to memory rather than failing: a Linux box without Secret Service still
/// runs Cogit, its tokens just do not outlive the process.
pub fn platform_store() -> Box<dyn SecretStore> {
    match keyring::Entry::store_status() {
        Ok(()) => Box::new(KeyringStore),
        Err(err) => {
            tracing::warn!(error = ?err, "no OS credential store; tokens will not persist");
            Box::new(MemoryStore::default())
        }
    }
}

/// The key a remote's token is stored under. `None` for a path, which needs no token.
pub fn host_of(url: &str) -> Option<String> {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest);
    let authority = match after_scheme {
        Some(rest) => rest.split(['/', '?', '#']).next()?,
        None => {
            let (candidate, _) = url.split_once(':')?;
            // `C:/work/repo` is a Windows path, not `host:path`.
            if candidate.len() < 2 {
                return None;
            }
            candidate
        }
    };

    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = host.split_once(':').map_or(host, |(h, _)| h);
    if host.is_empty() || !host.contains('.') {
        return None;
    }
    Some(host.to_owned())
}
