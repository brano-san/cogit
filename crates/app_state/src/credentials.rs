use crate::AppState;
use git_engine::Credential;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

const SERVICE: &str = "cogit";

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("a secret cannot be empty")]
    Empty,
    #[error("a sign-in is kept for an https:// host only")]
    NoHost,
    #[error("the credential store is unavailable: {0}")]
    Store(String),
}

/// The store is behind a trait so tests never touch the developer's real keychain, and so
/// a platform without one still gets a working application (doc/12-risks.md, R-06).
pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&self, key: &str, secret: &str) -> Result<(), SecretError>;
    fn delete(&self, key: &str) -> Result<(), SecretError>;
    /// `false`: what is stored is gone when Cogit exits.
    fn persistent(&self) -> bool;
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

    fn persistent(&self) -> bool {
        false
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

    fn persistent(&self) -> bool {
        true
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

/// The key a sign-in is kept under, and the only place that decides it: HTTPS, or plain
/// HTTP to this machine (over `http://` it would otherwise cross the network in clear); a
/// short name such as `gitlab` counts as a host, and the case does not matter. The
/// authority ends at the first `/`, `?` or `#`, so `https://h?a@evil/x` belongs to `h`;
/// userinfo and port are cut off (F-082: one sign-in per host, whatever the port or path).
#[must_use]
pub fn credential_host(url: &str) -> Option<String> {
    if let Some(rest) = url.strip_prefix("http://") {
        return host_part(rest).filter(|host| LOOPBACK.contains(&host.as_str()));
    }
    host_part(url.strip_prefix("https://")?)
}

const LOOPBACK: &[&str] = &["127.0.0.1", "[::1]", "localhost"];

/// What Preferences ▸ Authentication ▸ Add… accepts: a host, or a URL of one.
#[must_use]
pub fn host_input(text: &str) -> Option<String> {
    let text = text.trim();
    if text.chars().any(char::is_whitespace) {
        return None;
    }
    if text.contains("://") {
        credential_host(text)
    } else {
        credential_host(&format!("https://{text}"))
    }
}

fn host_part(rest: &str) -> Option<String> {
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = match host.strip_prefix('[') {
        // An IPv6 literal keeps its colons; only what follows the bracket is a port.
        Some(inner) => &host[..=inner.find(']')? + 1],
        None => host.split_once(':').map_or(host, |(h, _)| h),
    };
    (!host.is_empty()).then(|| host.to_lowercase())
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

/// What the Sign in dialog sends with a retry, and Preferences ▸ Authentication ▸ Add….
#[derive(Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SignIn {
    /// Used only for URLs of this host: a retry never hands it to another one.
    pub host: String,
    /// `None` or blank: `secret` is a token.
    pub username: Option<String>,
    pub secret: String,
    /// Kept once a command has succeeded with it.
    pub remember: bool,
}

impl std::fmt::Debug for SignIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignIn")
            .field("host", &self.host)
            .field("username", &self.username)
            .field("remember", &self.remember)
            .finish_non_exhaustive()
    }
}

impl SignIn {
    #[must_use]
    pub fn credential(&self) -> Credential {
        Credential {
            username: self
                .username
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_owned),
            secret: self.secret.clone(),
        }
    }
}

/// A host Cogit holds a sign-in for. Never the secret: that stays in the keyring.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KnownCredential {
    pub host: String,
    pub username: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CredentialList {
    /// `false`: no OS credential store, so a sign-in lasts until Cogit exits.
    pub persistent: bool,
    pub known: Vec<KnownCredential>,
}

/// One keyring entry per host, `{username, secret}` as JSON. An entry from before F-082
/// holds the bare token and reads as one without a username; it is never rewritten, so an
/// older Cogit still reads it.
#[derive(Serialize, Deserialize)]
struct Stored {
    username: Option<String>,
    secret: String,
}

/// The keyring cannot list its entries, so the hosts are listed in a file of their own.
#[derive(Default)]
pub(crate) struct KnownHosts {
    path: Option<PathBuf>,
    known: Vec<KnownCredential>,
}

impl KnownHosts {
    fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };
        let written = serde_json::to_vec_pretty(&self.known)
            .map_err(std::io::Error::other)
            .and_then(|bytes| std::fs::write(path, bytes));
        if let Err(err) = written {
            tracing::error!(error = ?err, path = %path.display(), context = "saving the list of sign-ins");
        }
    }

    fn put(&mut self, entry: KnownCredential) {
        if self.known.contains(&entry) {
            return;
        }
        self.known.retain(|known| known.host != entry.host);
        self.known.push(entry);
        self.known.sort_by(|a, b| a.host.cmp(&b.host));
        self.save();
    }
}

impl AppState {
    /// Where the list of hosts lives. Without an OS credential store nothing outlives the
    /// process, so the list is not written either: it would name sign-ins that are gone.
    pub fn use_credential_list(&self, path: PathBuf) {
        if !self.secrets.persistent() {
            return;
        }
        let known = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|err| {
                tracing::error!(error = ?err, path = %path.display(), context = "reading the list of sign-ins; starting empty");
                Vec::new()
            }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(err) => {
                tracing::error!(error = ?err, path = %path.display(), context = "reading the list of sign-ins; starting empty");
                Vec::new()
            }
        };
        *self.known_hosts.lock() = KnownHosts {
            path: Some(path),
            known,
        };
    }

    #[must_use]
    pub fn list_credentials(&self) -> CredentialList {
        CredentialList {
            persistent: self.secrets.persistent(),
            known: self.known_hosts.lock().known.clone(),
        }
    }

    pub fn store_credential(&self, sign_in: &SignIn) -> Result<(), SecretError> {
        let host = host_input(&sign_in.host).ok_or(SecretError::NoHost)?;
        let credential = sign_in.credential();
        if credential.secret.is_empty() {
            return Err(SecretError::Empty);
        }
        let stored = serde_json::to_string(&Stored {
            username: credential.username.clone(),
            secret: credential.secret,
        })
        .map_err(|err| SecretError::Store(err.to_string()))?;
        self.secrets.set(&host, &stored)?;
        self.known_hosts.lock().put(KnownCredential {
            host,
            username: credential.username,
        });
        Ok(())
    }

    pub fn forget_credential(&self, host: &str) -> Result<(), SecretError> {
        let host = host_input(host).ok_or(SecretError::NoHost)?;
        self.secrets.delete(&host)?;
        let mut known = self.known_hosts.lock();
        known.known.retain(|entry| entry.host != host);
        known.save();
        Ok(())
    }

    /// Every listed host goes from the list even when the keyring refuses one; the first
    /// refusal is the answer.
    pub fn forget_all_credentials(&self) -> Result<(), SecretError> {
        let mut known = self.known_hosts.lock();
        let mut first = Ok(());
        for entry in known.known.drain(..) {
            if let Err(err) = self.secrets.delete(&entry.host) {
                tracing::error!(error = ?err, host = %entry.host, context = "forgetting a sign-in");
                first = first.and(Err(err));
            }
        }
        known.save();
        first
    }

    /// What Cogit holds for the host of `url`. A host found in the keyring but missing from
    /// the list (a token stored before F-082) is listed now.
    pub fn stored_credential(&self, url: &str) -> Option<Credential> {
        let host = credential_host(url)?;
        let raw = self.secrets.get(&host)?;
        let credential = match serde_json::from_str::<Stored>(&raw) {
            Ok(stored) => Credential {
                username: stored.username,
                secret: stored.secret,
            },
            Err(_) => Credential {
                username: None,
                secret: raw,
            },
        };
        let mut known = self.known_hosts.lock();
        if !known.known.iter().any(|entry| entry.host == host) {
            known.put(KnownCredential {
                host,
                username: credential.username.clone(),
            });
        }
        Some(credential)
    }
}
