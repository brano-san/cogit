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
    /// Entries changed since the last write: written once the downloads settle, not once
    /// per picture — the index is the whole cache.
    dirty: AtomicBool,
    /// Four download threads store at once; one writer at a time, each with a snapshot
    /// taken inside, so an older one can never land after a newer one.
    writing: Mutex<()>,
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
            dirty: AtomicBool::new(false),
            writing: Mutex::new(()),
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
        // A deleted picture must not stay in the index on disk until the next settle.
        if self.evict() {
            self.save();
        } else {
            self.dirty.store(true, Ordering::SeqCst);
        }
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
        self.dirty.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Writes what the downloads stored since the last write. Called when they settle.
    pub fn settle(&self) {
        let _writing = self.writing.lock();
        if self.dirty.swap(false, Ordering::SeqCst) {
            self.touched.store(false, Ordering::SeqCst);
            self.write_index();
        }
    }

    /// Writes the use times as well. Called on drop.
    pub fn flush(&self) {
        let _writing = self.writing.lock();
        let dirty = self.dirty.swap(false, Ordering::SeqCst);
        if self.touched.swap(false, Ordering::SeqCst) || dirty {
            self.write_index();
        }
    }

    fn file(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.png"))
    }

    /// Oldest use first, until the pictures fit. Misses weigh nothing and are never evicted.
    /// Whether anything went.
    fn evict(&self) -> bool {
        let mut index = self.index.lock();
        let mut total: u64 = index.values().map(|e| e.bytes).sum();
        if total <= self.limit {
            return false;
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
        true
    }

    fn save(&self) {
        let _writing = self.writing.lock();
        self.dirty.store(false, Ordering::SeqCst);
        self.touched.store(false, Ordering::SeqCst);
        self.write_index();
    }

    /// Under `writing`. Through a temporary file: a write cut short, or read half-way by
    /// the next run, would cost the whole cache. Losing the index costs a refetch, never
    /// the run, so a failed write is only logged.
    fn write_index(&self) {
        let path = self.dir.join(INDEX);
        let partial = self.dir.join(format!("{INDEX}.partial"));
        let snapshot = self.index.lock().clone();
        let written = serde_json::to_vec(&snapshot)
            .map_err(std::io::Error::other)
            .and_then(|bytes| std::fs::write(&partial, bytes))
            .and_then(|()| std::fs::rename(&partial, &path));
        if let Err(error) = written {
            tracing::warn!(?error, ?path, "could not write the avatar cache index");
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
