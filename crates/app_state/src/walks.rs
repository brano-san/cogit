use crate::{AppState, RepoId};
use git_engine::{GitError, RepoHandle, WorktreeFiles};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

const SPARE_FOR: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Spare {
    Make,
    Take,
    Leave,
}

struct Walk {
    epoch: u64,
    done: OnceLock<Option<Arc<WorktreeFiles>>>,
    spare: AtomicBool,
    finished: OnceLock<Instant>,
}

impl Walk {
    fn joinable(&self, epoch: u64) -> bool {
        self.epoch == epoch && (self.done.get().is_none() || self.spare())
    }

    fn spare(&self) -> bool {
        self.spare.load(Ordering::Acquire)
            && self
                .finished
                .get()
                .is_some_and(|at| at.elapsed() < SPARE_FOR)
    }
}

#[derive(Default, Clone)]
pub(crate) struct Walks {
    epoch: Arc<AtomicU64>,
    by_root: Arc<parking_lot::Mutex<HashMap<PathBuf, Arc<Walk>>>>,
}

impl Walks {
    pub(crate) fn moved(&self) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
    }

    pub(crate) fn epoch(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.epoch)
    }

    pub(crate) fn start(&self, handle: RepoHandle) {
        let walks = self.clone();
        let spawned = std::thread::Builder::new()
            .name("status-walk".to_owned())
            .spawn(move || {
                if let Err(err) = walks.walk(&handle, Spare::Make) {
                    tracing::warn!(error = ?err, context = "status walk started by an open");
                }
            });
        if let Err(err) = spawned {
            tracing::error!(error = ?err, context = "could not start the status walk of an open");
        }
    }

    fn walk(&self, handle: &RepoHandle, spare: Spare) -> Result<Arc<WorktreeFiles>, GitError> {
        let epoch = self.epoch.load(Ordering::Acquire);
        let root = handle.root().to_path_buf();
        let (walk, leads) = {
            let mut by_root = self.by_root.lock();
            by_root.retain(|_, walk| walk.joinable(epoch));
            match by_root.get(&root) {
                Some(walk) if walk.done.get().is_none() => (Arc::clone(walk), false),
                Some(walk) if spare == Spare::Leave && walk.spare() => {
                    drop(by_root);
                    return handle.worktree_files().map(Arc::new);
                }
                Some(walk) if spare == Spare::Take && walk.spare.swap(false, Ordering::AcqRel) => {
                    if let Some(Some(files)) = walk.done.get() {
                        return Ok(Arc::clone(files));
                    }
                    let walk = Self::lead(epoch, spare);
                    by_root.insert(root, Arc::clone(&walk));
                    (walk, true)
                }
                _ => {
                    let walk = Self::lead(epoch, spare);
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
        let _ = walk.finished.set(Instant::now());
        let _ = walk.done.set(result.as_ref().ok().map(Arc::clone));
        result
    }

    fn lead(epoch: u64, spare: Spare) -> Arc<Walk> {
        Arc::new(Walk {
            epoch,
            done: OnceLock::new(),
            spare: AtomicBool::new(spare == Spare::Make),
            finished: OnceLock::new(),
        })
    }
}

impl AppState {
    pub(crate) fn walked(&self, repo: RepoId) -> Result<Arc<WorktreeFiles>, GitError> {
        self.walks.walk(&self.handle(repo)?, Spare::Leave)
    }

    pub(crate) fn walked_for_list(&self, repo: RepoId) -> Result<Arc<WorktreeFiles>, GitError> {
        self.walks.walk(&self.handle(repo)?, Spare::Take)
    }

    pub(crate) fn walked_at(&self, handle: &RepoHandle) -> Result<Arc<WorktreeFiles>, GitError> {
        self.walks.walk(handle, Spare::Leave)
    }
}
