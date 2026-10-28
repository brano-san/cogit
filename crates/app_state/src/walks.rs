use crate::{AppState, RepoId};
use git_engine::{GitError, RepoHandle, WorktreeFiles};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

struct Walk {
    epoch: u64,
    done: OnceLock<Option<Arc<WorktreeFiles>>>,
}

impl Walk {
    fn joinable(&self, epoch: u64) -> bool {
        self.epoch == epoch && self.done.get().is_none()
    }
}

#[derive(Default)]
pub(crate) struct Walks {
    epoch: Arc<AtomicU64>,
    by_root: parking_lot::Mutex<HashMap<PathBuf, Arc<Walk>>>,
}

impl Walks {
    pub(crate) fn moved(&self) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
    }

    pub(crate) fn epoch(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.epoch)
    }
}

impl AppState {
    pub(crate) fn walked(&self, repo: RepoId) -> Result<Arc<WorktreeFiles>, GitError> {
        self.walked_at(&self.handle(repo)?)
    }

    pub(crate) fn walked_at(&self, handle: &RepoHandle) -> Result<Arc<WorktreeFiles>, GitError> {
        let epoch = self.walks.epoch.load(Ordering::Acquire);
        let root = handle.root().to_path_buf();
        let (walk, leads) = {
            let mut by_root = self.walks.by_root.lock();
            by_root.retain(|_, walk| walk.joinable(epoch));
            match by_root.get(&root) {
                Some(walk) => (Arc::clone(walk), false),
                None => {
                    let walk = Arc::new(Walk {
                        epoch,
                        done: OnceLock::new(),
                    });
                    by_root.insert(root, Arc::clone(&walk));
                    (walk, true)
                }
            }
        };
        if !leads {
            if let Some(files) = walk.done.wait() {
                return Ok(Arc::clone(files));
            }
            return handle.worktree_files().map(Arc::new);
        }
        let result = handle.worktree_files().map(Arc::new);
        let _ = walk.done.set(result.as_ref().ok().map(Arc::clone));
        result
    }
}
