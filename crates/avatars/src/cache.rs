//! Pictures on disk, flat, plus one index that remembers misses as well as hits.

use crate::identity::email_hash;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

const FOUND_TTL: u64 = 30 * 24 * 60 * 60;
const MISSING_TTL: u64 = 7 * 24 * 60 * 60;
const DEFAULT_LIMIT: u64 = 64 * 1024 * 1024;
const INDEX: &str = "index.json";

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("avatar cache at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup {
    Hit(PathBuf),
    /// Asked for and not there. Kept so the graph does not ask again every repaint.
    Missing,
    /// Never asked, or asked too long ago to trust.
    Unknown,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Entry {
    fetched_at: u64,
    used_at: u64,
    #[serde(default)]
    bytes: u64,
    #[serde(default)]
    missing: bool,
}

type Clock = Box<dyn Fn() -> u64 + Send + Sync>;

pub struct Cache {
    dir: PathBuf,
    index: Mutex<HashMap<String, Entry>>,
    limit: u64,
    clock: Clock,
    /// Use times changed since the last write. They only order eviction, so they are
    /// worth a flush at the end, never a file write per row of a scrolling list.
    touched: AtomicBool,
}

impl std::fmt::Debug for Cache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cache")
            .field("dir", &self.dir)
            .field("limit", &self.limit)
            .finish_non_exhaustive()
    }
}

impl Cache {
    /// Creating the directory is what "avatars are on" means; `off` never calls this.
    pub fn open(dir: PathBuf) -> Result<Self, CacheError> {
        std::fs::create_dir_all(&dir).map_err(|source| CacheError::Io {
            path: dir.clone(),
            source,
        })?;
        let index = read_index(&dir.join(INDEX));
        Ok(Self {
            dir,
            index: Mutex::new(index),
            limit: DEFAULT_LIMIT,
            clock: Box::new(now),
            touched: AtomicBool::new(false),
        })
    }

    pub fn with_limit(mut self, bytes: u64) -> Self {
        self.limit = bytes;
        self
    }

    pub fn with_clock(mut self, clock: impl Fn() -> u64 + Send + Sync + 'static) -> Self {
        self.clock = Box::new(clock);
        self
    }

    pub fn lookup(&self, email: &str) -> Lookup {
        let key = email_hash(email);
        let now = (self.clock)();
        let mut index = self.index.lock();

        let Some(entry) = index.get_mut(&key) else {
            return Lookup::Unknown;
        };
        let ttl = if entry.missing {
            MISSING_TTL
        } else {
            FOUND_TTL
        };
        if now.saturating_sub(entry.fetched_at) >= ttl {
            let missing = entry.missing;
            index.remove(&key);
            if !missing {
                let _ = std::fs::remove_file(self.file(&key));
            }
            drop(index);
            self.save();
            return Lookup::Unknown;
        }

        if entry.missing {
            return Lookup::Missing;
        }
        entry.used_at = now;
        let path = self.file(&key);
        drop(index);
        self.touched.store(true, Ordering::Relaxed);
        Lookup::Hit(path)
    }

    pub fn store(&self, email: &str, bytes: &[u8]) -> Result<PathBuf, CacheError> {
        let key = email_hash(email);
        let path = self.file(&key);
        std::fs::write(&path, bytes).map_err(|source| CacheError::Io {
            path: path.clone(),
            source,
        })?;

        let now = (self.clock)();
        self.index.lock().insert(
            key,
            Entry {
                fetched_at: now,
                used_at: now,
                bytes: bytes.len() as u64,
                missing: false,
            },
        );
        self.evict();
        self.save();
        Ok(path)
    }

    pub fn store_missing(&self, email: &str) -> Result<(), CacheError> {
        let now = (self.clock)();
        self.index.lock().insert(
            email_hash(email),
            Entry {
                fetched_at: now,
                used_at: now,
                bytes: 0,
                missing: true,
            },
        );
        self.save();
        Ok(())
    }

    /// Writes the use times if any changed. Called when the window settles, and on drop.
    pub fn flush(&self) {
        if self.touched.swap(false, Ordering::Relaxed) {
            self.save();
        }
    }

    fn file(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.png"))
    }

    /// Oldest use first, until the pictures fit. Misses weigh nothing and are never evicted.
    fn evict(&self) {
        let mut index = self.index.lock();
        let mut total: u64 = index.values().map(|e| e.bytes).sum();
        if total <= self.limit {
            return;
        }

        let mut order: Vec<(String, u64, u64)> = index
            .iter()
            .filter(|(_, e)| !e.missing)
            .map(|(k, e)| (k.clone(), e.used_at, e.bytes))
            .collect();
        order.sort_by_key(|(_, used_at, _)| *used_at);

        for (key, _, bytes) in order {
            if total <= self.limit {
                break;
            }
            let _ = std::fs::remove_file(self.file(&key));
            index.remove(&key);
            total = total.saturating_sub(bytes);
        }
    }

    /// Losing the index costs a refetch, never the run, so a failed write is only logged.
    fn save(&self) {
        self.touched.store(false, Ordering::Relaxed);
        let path = self.dir.join(INDEX);
        let snapshot = self.index.lock().clone();
        match serde_json::to_vec(&snapshot) {
            Ok(bytes) => {
                if let Err(error) = std::fs::write(&path, bytes) {
                    tracing::warn!(?error, ?path, "could not write the avatar cache index");
                }
            }
            Err(error) => tracing::warn!(?error, "could not serialise the avatar cache index"),
        }
    }
}

impl Drop for Cache {
    fn drop(&mut self) {
        self.flush();
    }
}

fn read_index(path: &Path) -> HashMap<String, Entry> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
            tracing::warn!(
                ?error,
                ?path,
                "avatar cache index unreadable, starting over"
            );
            HashMap::new()
        }),
        Err(_) => HashMap::new(),
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
