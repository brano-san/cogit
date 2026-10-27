use crate::AppState;
use crate::password_store::{PasswordStore, StoreError, StoreState};
use git_engine::Credential;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use zeroize::Zeroizing;

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("a password or token cannot be empty")]
    Empty,
    #[error("a sign-in is kept for an https:// or ssh host only")]
    NoHost,
    #[error("Cogit's password store is not set up")]
    NoStore,
    #[error(transparent)]
    Store(#[from] StoreError),
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

/// The host of an ssh remote: `ssh://`, `git+ssh://`, or scp-like `user@host:path`.
#[must_use]
pub fn ssh_host(url: &str) -> Option<String> {
    for scheme in ["ssh://", "git+ssh://", "ssh+git://"] {
        if let Some(rest) = url.strip_prefix(scheme) {
            return host_part(rest);
        }
    }
    if url.contains("://") {
        return None;
    }
    let (authority, _) = url.split_once(':')?;
    // `C:/work/repo` is a Windows path, `./a:b` a relative one.
    if authority.len() < 2 || authority.contains(['/', '\\']) {
        return None;
    }
    host_part(authority)
}

#[must_use]
pub fn credential_target(url: &str) -> Option<(String, bool)> {
    credential_host(url)
        .map(|host| (host, false))
        .or_else(|| ssh_host(url).map(|host| (host, true)))
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
        credential_target(text).map(|(host, _)| host)
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
#[derive(Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SignIn {
    /// Used only for URLs of this host: a retry never hands it to another one.
    pub host: String,
    /// `None` or blank: `secret` is a token.
    pub username: Option<String>,
    /// The password or token; with `key_path`, the key's passphrase (may be empty).
    pub secret: String,
    /// An ssh private key: the sign-in is for the host's ssh remotes.
    #[serde(default)]
    pub key_path: Option<String>,
    /// Kept once a command has succeeded with it.
    pub remember: bool,
}

impl std::fmt::Debug for SignIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignIn")
            .field("host", &self.host)
            .field("username", &self.username)
            .field("key_path", &self.key_path)
            .field("remember", &self.remember)
            .finish_non_exhaustive()
    }
}

impl SignIn {
    #[must_use]
    pub fn is_ssh(&self) -> bool {
        self.key().is_some()
    }

    fn key(&self) -> Option<String> {
        self.key_path
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(str::to_owned)
    }

    #[must_use]
    pub fn credential(&self) -> Credential {
        Credential {
            username: self
                .username
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty() && self.key().is_none())
                .map(str::to_owned),
            secret: self.secret.clone(),
            key: self.key(),
        }
    }
}

/// A sign-in Cogit holds. Never the secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KnownCredential {
    pub host: String,
    pub username: Option<String>,
    /// An ssh key's path: the sign-in is for the host's ssh remotes.
    pub key_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStore {
    pub state: StoreState,
    /// `false` also while there is no store yet.
    pub master_password: bool,
    /// Empty while locked: the list is inside the encrypted file.
    pub known: Vec<KnownCredential>,
}

/// `~/.ssh` and the private keys in it, for the Sign in dialog's SSH key tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SshKeys {
    pub dir: String,
    pub keys: Vec<String>,
}

const STANDARD_KEYS: &[&str] = &["id_ed25519", "id_ecdsa", "id_rsa"];

#[must_use]
pub fn ssh_keys() -> SshKeys {
    let dir = std::env::home_dir().unwrap_or_default().join(".ssh");
    SshKeys {
        keys: private_keys(&dir),
        dir: dir.to_string_lossy().into_owned(),
    }
}

/// Files that begin as a private key does; never `.pub`, `known_hosts`, `config` and the
/// like. The standard names first, as ssh tries them, then the rest by name.
#[must_use]
pub fn private_keys(dir: &std::path::Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<(usize, String, String)> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".pub") {
                return None;
            }
            let mut head = [0u8; 64];
            let read = std::io::Read::read(&mut std::fs::File::open(entry.path()).ok()?, &mut head)
                .ok()?;
            let head = String::from_utf8_lossy(&head[..read]);
            if !(head.starts_with("-----BEGIN") && head.contains("PRIVATE KEY")) {
                return None;
            }
            let rank = STANDARD_KEYS
                .iter()
                .position(|standard| *standard == name)
                .unwrap_or(STANDARD_KEYS.len());
            Some((rank, name, entry.path().to_string_lossy().into_owned()))
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, _, path)| path).collect()
}

#[derive(Serialize, Deserialize)]
struct Stored {
    host: String,
    username: Option<String>,
    secret: String,
    #[serde(default)]
    key: Option<String>,
}

impl Stored {
    fn is(&self, host: &str, ssh: bool) -> bool {
        self.host == host && self.key.is_some() == ssh
    }
}

fn parse(plain: &[u8]) -> Result<Vec<Stored>, SecretError> {
    serde_json::from_slice(plain).map_err(|err| StoreError::Damaged(err.to_string()).into())
}

fn serialize(entries: &[Stored]) -> Result<Zeroizing<Vec<u8>>, StoreError> {
    serde_json::to_vec(entries)
        .map(Zeroizing::new)
        .map_err(|err| StoreError::Damaged(err.to_string()))
}

pub(crate) type Passwords = Arc<RwLock<Option<PasswordStore>>>;

fn with_store_in<T>(
    passwords: &Passwords,
    run: impl FnOnce(&PasswordStore) -> Result<T, SecretError>,
) -> Result<T, SecretError> {
    let store = passwords.read();
    run(store.as_ref().ok_or(SecretError::NoStore)?)
}

fn change_entries_in(
    passwords: &Passwords,
    change: impl FnOnce(&mut Vec<Stored>),
) -> Result<(), SecretError> {
    with_store_in(passwords, |store| {
        store
            .update(|plain| {
                let mut entries = parse(plain).map_err(|err| match err {
                    SecretError::Store(err) => err,
                    other => StoreError::Damaged(other.to_string()),
                })?;
                change(&mut entries);
                serialize(&entries)
            })
            .map_err(SecretError::from)
    })
}

pub(crate) fn store_in(passwords: &Passwords, sign_in: &SignIn) -> Result<(), SecretError> {
    let host = host_input(&sign_in.host).ok_or(SecretError::NoHost)?;
    let credential = sign_in.credential();
    let ssh = credential.key.is_some();
    if credential.secret.is_empty() && !ssh {
        return Err(SecretError::Empty);
    }
    change_entries_in(passwords, |entries| {
        entries.retain(|entry| !entry.is(&host, ssh));
        entries.push(Stored {
            host,
            username: credential.username,
            secret: credential.secret,
            key: credential.key,
        });
    })
}

pub(crate) fn stored_in(passwords: &Passwords, url: &str) -> Option<Credential> {
    let (host, ssh) = credential_target(url)?;
    let entries = match with_store_in(passwords, |store| parse(&store.read()?)) {
        Ok(entries) => entries,
        Err(SecretError::Store(StoreError::Locked) | SecretError::NoStore) => return None,
        Err(err) => {
            tracing::error!(error = ?err, context = "reading the password store");
            return None;
        }
    };
    entries
        .into_iter()
        .find(|entry| entry.is(&host, ssh))
        .map(|entry| Credential {
            username: entry.username,
            secret: entry.secret,
            key: entry.key,
        })
}

impl AppState {
    pub fn use_password_store(&self, path: PathBuf) {
        *self.passwords.write() = Some(PasswordStore::new(path));
    }

    fn with_store<T>(
        &self,
        run: impl FnOnce(&PasswordStore) -> Result<T, SecretError>,
    ) -> Result<T, SecretError> {
        with_store_in(&self.passwords, run)
    }

    fn change_entries(&self, change: impl FnOnce(&mut Vec<Stored>)) -> Result<(), SecretError> {
        change_entries_in(&self.passwords, change)
    }

    pub fn credential_store(&self) -> Result<CredentialStore, SecretError> {
        self.with_store(|store| {
            let state = store.state();
            let known = if state == StoreState::Unlocked {
                let mut known: Vec<KnownCredential> = parse(&store.read()?)?
                    .into_iter()
                    .map(|entry| KnownCredential {
                        host: entry.host,
                        username: entry.username,
                        key_path: entry.key,
                    })
                    .collect();
                known.sort_by(|a, b| (&a.host, &a.key_path).cmp(&(&b.host, &b.key_path)));
                known
            } else {
                Vec::new()
            };
            Ok(CredentialStore {
                state,
                master_password: store.has_master_password().unwrap_or(false),
                known,
            })
        })
    }

    pub fn unlock_credentials(&self, password: &str) -> Result<bool, SecretError> {
        let password = Zeroizing::new(password.to_owned());
        self.with_store(|store| Ok(store.unlock(&password)?))
    }

    /// The first store; `None` is "Don't use a master password".
    pub fn create_credential_store(&self, password: Option<&str>) -> Result<(), SecretError> {
        self.with_store(|store| Ok(store.create(password)?))
    }

    pub fn change_master_password(
        &self,
        current: &str,
        new: Option<&str>,
    ) -> Result<bool, SecretError> {
        self.with_store(|store| Ok(store.change_password(current, new)?))
    }

    pub fn reset_credential_store(&self, new: Option<&str>) -> Result<(), SecretError> {
        self.with_store(|store| Ok(store.reset(new)?))
    }

    pub fn store_credential(&self, sign_in: &SignIn) -> Result<(), SecretError> {
        if let Some(window) = self.window.get() {
            crate::agent_events::ask_window::<()>(
                window,
                &crate::agent_events::Question::Remember {
                    sign_in: sign_in.clone(),
                },
            );
            return Ok(());
        }
        store_in(&self.passwords, sign_in)
    }

    pub fn forget_credential(&self, host: &str, ssh: bool) -> Result<(), SecretError> {
        let host = host_input(host).ok_or(SecretError::NoHost)?;
        self.change_entries(|entries| entries.retain(|entry| !entry.is(&host, ssh)))
    }

    pub fn forget_all_credentials(&self) -> Result<(), SecretError> {
        self.change_entries(Vec::clear)
    }

    /// What Cogit holds for the host of `url`, of `url`'s kind. A locked store holds nothing
    /// yet: the page unlocks it before a command that may ask (R-891).
    pub fn stored_credential(&self, url: &str) -> Option<Credential> {
        if let Some(window) = self.window.get() {
            return crate::agent_events::ask_window(
                window,
                &crate::agent_events::Question::Credential {
                    url: url.to_owned(),
                },
            );
        }
        stored_in(&self.passwords, url)
    }

    /// Moves the keychain sign-ins listed in `list` into a new store without a master
    /// password, once; an unlisted host is not found (R-891).
    pub fn move_keychain_sign_ins(&self, list: &std::path::Path) {
        let Ok(bytes) = std::fs::read(list) else {
            return;
        };
        let hosts: Vec<KnownCredential> = serde_json::from_slice(&bytes).unwrap_or_else(|err| {
            tracing::error!(error = ?err, context = "reading the keychain's list of sign-ins");
            Vec::new()
        });
        if !matches!(
            self.credential_store().map(|store| store.state),
            Ok(StoreState::Missing)
        ) {
            return;
        }
        let found: Vec<Stored> = hosts
            .iter()
            .filter_map(|known| keychain::take(&known.host))
            .collect();
        tracing::info!(
            listed = hosts.len(),
            moved = found.len(),
            "moving keychain sign-ins"
        );
        let moved = self
            .create_credential_store(None)
            .and_then(|()| self.change_entries(|entries| entries.extend(found)));
        match moved {
            Ok(()) => {
                for known in &hosts {
                    keychain::delete(&known.host);
                }
                if let Err(err) = std::fs::remove_file(list) {
                    tracing::error!(error = ?err, context = "removing the keychain's list of sign-ins");
                }
            }
            Err(err) => tracing::error!(error = ?err, context = "moving keychain sign-ins"),
        }
    }
}

/// Before R-891: one keychain entry per host, `{username, secret}` JSON or a bare token.
mod keychain {
    use super::Stored;

    fn entry(host: &str) -> Option<keyring::Entry> {
        keyring::Entry::new("cogit", host)
            .map_err(|err| tracing::warn!(error = ?err, "no system keychain to move sign-ins from"))
            .ok()
    }

    pub(super) fn take(host: &str) -> Option<Stored> {
        let raw = match entry(host)?.get_password() {
            Ok(raw) => raw,
            Err(keyring::Error::NoEntry) => return None,
            Err(err) => {
                tracing::error!(error = ?err, host, context = "reading a keychain sign-in");
                return None;
            }
        };
        #[derive(serde::Deserialize)]
        struct Old {
            username: Option<String>,
            secret: String,
        }
        let old = serde_json::from_str::<Old>(&raw).unwrap_or(Old {
            username: None,
            secret: raw,
        });
        Some(Stored {
            host: host.to_owned(),
            username: old.username,
            secret: old.secret,
            key: None,
        })
    }

    pub(super) fn delete(host: &str) {
        if let Some(Err(err)) = entry(host).map(|entry| entry.delete_credential())
            && !matches!(err, keyring::Error::NoEntry)
        {
            tracing::error!(error = ?err, host, context = "removing a moved keychain sign-in");
        }
    }
}
