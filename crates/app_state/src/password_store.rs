//! Cogit's own password store (R-891): one file in the settings folder, XChaCha20-Poly1305
//! under a key Argon2id derives from the master password. "No master password" is the
//! empty one: the file is still encrypted, but anyone who can read it can open it.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

const VERSION: u32 = 1;
/// OWASP's first Argon2id line is 19 MiB, t=2; a little more costs one pause per session.
const KDF: Kdf = Kdf {
    memory_kib: 65_536,
    iterations: 3,
    lanes: 1,
};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("the password store is locked: enter the master password first")]
    Locked,
    #[error("the password store already exists")]
    Exists,
    #[error("the password store file is version {0}; this Cogit reads version {VERSION}")]
    Version(u32),
    #[error("the password store file is damaged: {0}")]
    Damaged(String),
    #[error("the password store file was changed by another Cogit: unlock it again")]
    Replaced,
    #[error("the password store could not be written: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Kdf {
    memory_kib: u32,
    iterations: u32,
    lanes: u32,
}

/// What is on disk. Everything but `data` is the AEAD's associated data, so a header edited
/// by hand fails to decrypt instead of being believed.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct File {
    version: u32,
    master_password: bool,
    kdf: Kdf,
    salt: String,
    nonce: String,
    data: String,
}

impl File {
    fn aad(&self) -> Vec<u8> {
        format!(
            "cogit-password-store/{}/{}/{}/{}/{}/{}",
            self.version,
            self.master_password,
            self.kdf.memory_kib,
            self.kdf.iterations,
            self.kdf.lanes,
            self.salt
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum StoreState {
    /// Nothing stored yet: the first Remember sets the master password (or none).
    Missing,
    Locked,
    Unlocked,
}

struct Open {
    key: Zeroizing<[u8; 32]>,
    salt: String,
}

#[derive(Debug)]
pub struct PasswordStore {
    path: PathBuf,
    open: Mutex<Option<Open>>,
}

impl std::fmt::Debug for Open {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Open")
    }
}

impl PasswordStore {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            open: Mutex::new(None),
        }
    }

    #[cfg(test)]
    fn path(&self) -> &Path {
        &self.path
    }

    /// A store without a master password opens itself here: there is nothing to ask.
    pub fn state(&self) -> StoreState {
        let Ok(Some(file)) = self.load() else {
            return if self.path.exists() {
                StoreState::Locked
            } else {
                StoreState::Missing
            };
        };
        if self
            .open
            .lock()
            .as_ref()
            .is_some_and(|o| o.salt == file.salt)
        {
            return StoreState::Unlocked;
        }
        if !file.master_password && self.unlock_file(&file, "").unwrap_or(false) {
            return StoreState::Unlocked;
        }
        StoreState::Locked
    }

    /// Whether the store asks for a master password; `None` while there is no store.
    #[must_use]
    pub fn has_master_password(&self) -> Option<bool> {
        self.load().ok().flatten().map(|file| file.master_password)
    }

    /// `false`: a wrong password; the store stays locked.
    pub fn unlock(&self, password: &str) -> Result<bool, StoreError> {
        let file = self.load()?.ok_or(StoreError::Locked)?;
        self.unlock_file(&file, password)
    }

    fn unlock_file(&self, file: &File, password: &str) -> Result<bool, StoreError> {
        let key = derive(password, &file.salt, file.kdf)?;
        if decrypt(file, &key).is_err() {
            return Ok(false);
        }
        *self.open.lock() = Some(Open {
            key,
            salt: file.salt.clone(),
        });
        Ok(true)
    }

    /// The first store, empty. `None`: no master password.
    pub fn create(&self, password: Option<&str>) -> Result<(), StoreError> {
        if self.path.exists() {
            return Err(StoreError::Exists);
        }
        self.write_new(password, b"[]")
    }

    /// The plaintext; empty while there is no store.
    pub fn read(&self) -> Result<Zeroizing<Vec<u8>>, StoreError> {
        let Some(file) = self.load()? else {
            return Ok(Zeroizing::new(b"[]".to_vec()));
        };
        if self.state() != StoreState::Unlocked {
            // Another Cogit set a new master password: the old key opens nothing now.
            *self.open.lock() = None;
            return Err(StoreError::Locked);
        }
        let open = self.open.lock();
        let open = open.as_ref().ok_or(StoreError::Locked)?;
        decrypt(&file, &open.key).map_err(|()| StoreError::Replaced)
    }

    /// Read, change, write back whole; the file on disk is re-read first, so a change made
    /// by another Cogit since is kept. ponytail: no lock across processes, the last writer of
    /// two simultaneous changes wins; add a lock file if that ever matters.
    pub fn update(
        &self,
        change: impl FnOnce(&[u8]) -> Result<Zeroizing<Vec<u8>>, StoreError>,
    ) -> Result<(), StoreError> {
        let Some(file) = self.load()? else {
            return Err(StoreError::Locked);
        };
        let plain = self.read()?;
        let changed = change(&plain)?;
        let open = self.open.lock();
        let open = open.as_ref().ok_or(StoreError::Locked)?;
        self.write(
            file.master_password,
            file.kdf,
            &file.salt,
            &open.key,
            &changed,
        )
    }

    /// Keeps what is stored; `false`: `current` is not the master password.
    pub fn change_password(&self, current: &str, new: Option<&str>) -> Result<bool, StoreError> {
        let file = self.load()?.ok_or(StoreError::Locked)?;
        let key = derive(current, &file.salt, file.kdf)?;
        let Ok(plain) = decrypt(&file, &key) else {
            return Ok(false);
        };
        self.write_new(new, &plain)?;
        Ok(true)
    }

    /// The master password is forgotten: a new one (or none), and everything stored is gone.
    pub fn reset(&self, new: Option<&str>) -> Result<(), StoreError> {
        self.write_new(new, b"[]")
    }

    fn write_new(&self, password: Option<&str>, plain: &[u8]) -> Result<(), StoreError> {
        let salt = hex(&random::<16>()?);
        let key = derive(password.unwrap_or(""), &salt, KDF)?;
        self.write(password.is_some(), KDF, &salt, &key, plain)?;
        *self.open.lock() = Some(Open { key, salt });
        Ok(())
    }

    fn write(
        &self,
        master_password: bool,
        kdf: Kdf,
        salt: &str,
        key: &[u8; 32],
        plain: &[u8],
    ) -> Result<(), StoreError> {
        let nonce = random::<24>()?;
        let mut file = File {
            version: VERSION,
            master_password,
            kdf,
            salt: salt.to_owned(),
            nonce: hex(&nonce),
            data: String::new(),
        };
        let aad = file.aad();
        let sealed = XChaCha20Poly1305::new(key.into())
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: plain,
                    aad: &aad,
                },
            )
            .map_err(|err| StoreError::Damaged(err.to_string()))?;
        file.data = hex(&sealed);
        let bytes =
            serde_json::to_vec_pretty(&file).map_err(|err| StoreError::Damaged(err.to_string()))?;
        replace(&self.path, &bytes)?;
        Ok(())
    }

    fn load(&self) -> Result<Option<File>, StoreError> {
        let bytes = match std::fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        #[derive(Deserialize)]
        struct Versioned {
            version: u32,
        }
        let versioned: Versioned =
            serde_json::from_slice(&bytes).map_err(|err| StoreError::Damaged(err.to_string()))?;
        if versioned.version != VERSION {
            return Err(StoreError::Version(versioned.version));
        }
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|err| StoreError::Damaged(err.to_string()))
    }
}

fn derive(password: &str, salt: &str, kdf: Kdf) -> Result<Zeroizing<[u8; 32]>, StoreError> {
    let params = Params::new(kdf.memory_kib, kdf.iterations, kdf.lanes, Some(32))
        .map_err(|err| StoreError::Damaged(err.to_string()))?;
    let salt = unhex(salt).ok_or_else(|| StoreError::Damaged("salt".to_owned()))?;
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(password.as_bytes(), &salt, key.as_mut())
        .map_err(|err| StoreError::Damaged(err.to_string()))?;
    Ok(key)
}

fn decrypt(file: &File, key: &[u8; 32]) -> Result<Zeroizing<Vec<u8>>, ()> {
    let nonce = unhex(&file.nonce).filter(|n| n.len() == 24).ok_or(())?;
    let data = unhex(&file.data).ok_or(())?;
    XChaCha20Poly1305::new(key.into())
        .decrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &data,
                aad: &file.aad(),
            },
        )
        .map(Zeroizing::new)
        .map_err(drop)
}

fn random<const N: usize>() -> Result<[u8; N], StoreError> {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes)
        .map_err(|err| StoreError::Io(std::io::Error::other(err.to_string())))?;
    Ok(bytes)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(text.get(at..at + 2)?, 16).ok())
        .collect()
}

/// A temporary file beside it, then a rename: a crash leaves the old store or the new one,
/// never half of either. Owner-only on Unix from the moment it exists.
fn replace(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let written = options.open(&temp).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()
    });
    if let Err(err) = written.and_then(|()| std::fs::rename(&temp, path)) {
        let _ = std::fs::remove_file(&temp);
        return Err(err);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn store() -> (tempfile::TempDir, PasswordStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = PasswordStore::new(dir.path().join("passwords.json"));
        (dir, store)
    }

    fn put(store: &PasswordStore, text: &str) {
        store
            .update(|_| Ok(Zeroizing::new(text.as_bytes().to_vec())))
            .unwrap();
    }

    fn reopen(store: &PasswordStore) -> PasswordStore {
        PasswordStore::new(store.path().to_path_buf())
    }

    #[test]
    fn what_is_written_reads_back_after_a_restart_and_is_not_in_the_file() {
        let (_dir, store) = store();
        store.create(Some("master")).unwrap();
        put(&store, "s3cret-value");

        let raw = std::fs::read_to_string(store.path()).unwrap();
        assert!(!raw.contains("s3cret"), "{raw}");
        let again = reopen(&store);
        assert_eq!(again.state(), StoreState::Locked);
        assert!(again.unlock("master").unwrap());
        assert_eq!(again.read().unwrap().as_slice(), b"s3cret-value");
    }

    #[test]
    fn a_wrong_master_password_leaves_it_locked() {
        let (_dir, store) = store();
        store.create(Some("master")).unwrap();
        let again = reopen(&store);

        assert!(!again.unlock("wrong").unwrap());
        assert_eq!(again.state(), StoreState::Locked);
        assert!(matches!(again.read(), Err(StoreError::Locked)));
    }

    #[test]
    fn without_a_master_password_it_opens_by_itself() {
        let (_dir, store) = store();
        assert_eq!(store.state(), StoreState::Missing);
        store.create(None).unwrap();
        put(&store, "x");

        let again = reopen(&store);
        assert_eq!(again.state(), StoreState::Unlocked);
        assert_eq!(again.read().unwrap().as_slice(), b"x");
        assert_eq!(again.has_master_password(), Some(false));
    }

    #[test]
    fn changing_the_master_password_keeps_the_data() {
        let (_dir, store) = store();
        store.create(Some("old")).unwrap();
        put(&store, "kept");

        assert!(!store.change_password("not-it", Some("new")).unwrap());
        assert!(store.change_password("old", Some("new")).unwrap());

        let again = reopen(&store);
        assert!(!again.unlock("old").unwrap());
        assert!(again.unlock("new").unwrap());
        assert_eq!(again.read().unwrap().as_slice(), b"kept");
    }

    #[test]
    fn a_forgotten_master_password_starts_over_empty() {
        let (_dir, store) = store();
        store.create(Some("old")).unwrap();
        put(&store, "gone");
        let again = reopen(&store);

        again.reset(Some("new")).unwrap();

        assert_eq!(again.read().unwrap().as_slice(), b"[]");
        let third = reopen(&store);
        assert!(third.unlock("new").unwrap());
    }

    #[test]
    fn a_file_of_another_version_is_refused_not_overwritten() {
        let (_dir, store) = store();
        store.create(None).unwrap();
        let raw = std::fs::read_to_string(store.path()).unwrap();
        std::fs::write(
            store.path(),
            raw.replace("\"version\": 1", "\"version\": 2"),
        )
        .unwrap();

        let again = reopen(&store);
        assert!(matches!(again.unlock(""), Err(StoreError::Version(2))));
        assert!(matches!(again.create(None), Err(StoreError::Exists)));
    }

    #[test]
    fn an_edited_header_does_not_decrypt() {
        let (_dir, store) = store();
        store.create(Some("m")).unwrap();
        let raw = std::fs::read_to_string(store.path()).unwrap();
        std::fs::write(
            store.path(),
            raw.replace("\"masterPassword\": true", "\"masterPassword\": false"),
        )
        .unwrap();

        assert_eq!(reopen(&store).state(), StoreState::Locked);
    }

    // Two Cogits on one store: a change made by the other is read back before writing, and
    // the file is replaced whole, never left half-written or beside a temporary.
    #[test]
    fn a_change_by_another_process_is_kept_and_the_file_is_replaced_whole() {
        let (dir, first) = store();
        first.create(None).unwrap();
        let second = reopen(&first);
        put(&second, "from-second");

        first
            .update(|old| {
                let mut both = old.to_vec();
                both.extend_from_slice(b"+first");
                Ok(Zeroizing::new(both))
            })
            .unwrap();

        assert_eq!(second.read().unwrap().as_slice(), b"from-second+first");
        let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn a_store_reset_elsewhere_must_be_unlocked_again() {
        let (_dir, first) = store();
        first.create(Some("a")).unwrap();
        let second = reopen(&first);
        second.reset(Some("b")).unwrap();

        assert!(matches!(first.read(), Err(StoreError::Locked)));
        assert_eq!(first.state(), StoreState::Locked);
    }

    #[cfg(unix)]
    #[test]
    fn the_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt as _;
        let (_dir, store) = store();
        store.create(None).unwrap();
        let mode = std::fs::metadata(store.path())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
