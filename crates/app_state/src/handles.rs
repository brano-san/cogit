//! Each open repository is opened from disk once and handed out per call, instead of
//! being discovered and its config parsed again by every command (doc/12-risks.md, R-324).

use crate::RepoId;
use git_engine::{GitError, RepoHandle, SharedRepo};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

#[derive(Default)]
pub(crate) struct HandleCache {
    held: Mutex<HashMap<RepoId, Arc<Held>>>,
    opened: AtomicU32,
    /// A call that began before a `forget` must not put the forgotten entry back.
    forgets: AtomicU64,
}

struct Held {
    root: PathBuf,
    shared: SharedRepo,
}

impl HandleCache {
    /// Reopens when the root is not the one held or a config file has changed since.
    pub(crate) fn handle(&self, repo: RepoId, root: &Path) -> Result<RepoHandle, GitError> {
        let since = self.forgets.load(Ordering::SeqCst);
        let held = self.held.lock().get(&repo).cloned();
        if let Some(held) = held
            && held.root == root
            && held.shared.is_current()
        {
            return held.shared.handle();
        }

        let shared = match SharedRepo::open(root) {
            Ok(shared) => shared,
            Err(err) => {
                self.held.lock().remove(&repo);
                return Err(err);
            }
        };
        self.opened.fetch_add(1, Ordering::Relaxed);
        let held = Arc::new(Held {
            root: root.to_path_buf(),
            shared,
        });
        let handle = held.shared.handle();
        let mut map = self.held.lock();
        if self.forgets.load(Ordering::SeqCst) == since {
            map.insert(repo, held);
        }
        handle
    }

    pub(crate) fn forget(&self, repo: RepoId) {
        let mut map = self.held.lock();
        self.forgets.fetch_add(1, Ordering::SeqCst);
        map.remove(&repo);
    }

    pub(crate) fn opened(&self) -> u32 {
        self.opened.load(Ordering::Relaxed)
    }

    pub(crate) fn held(&self) -> usize {
        self.held.lock().len()
    }
}
