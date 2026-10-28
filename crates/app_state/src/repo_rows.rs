//! Rows of the Repositories list not on screen, by folder: a closed one has no `RepoId`.

use git_engine::{GitError, RepoHandle, Submodule};
use std::path::Path;

/// `parent` is a key from the top, `""` for the top (R-352).
pub fn submodule_outline(root: &Path, parent: &str) -> Result<Vec<Submodule>, GitError> {
    let handle = if parent.is_empty() {
        RepoHandle::open_root(root)?
    } else {
        RepoHandle::open_exact(&root.join(parent))?
    };
    handle.submodule_outline()
}

#[must_use]
pub fn pulse(root: &Path) -> git_engine::RepoPulse {
    git_engine::pulse(root)
}

impl crate::AppState {
    /// Asks the server, writes nothing; a failure is logged and means "unknown" (R-354).
    /// Sends what is stored for the host, never asks (F-082).
    pub fn pull_probe(&self, root: &Path) -> Result<Option<bool>, GitError> {
        let probed = self
            .on_host_of(root, |root| crate::backend::Request::PullProbe { root })
            .unwrap_or_else(|| {
                RepoHandle::open_root(root).and_then(|handle| handle.pull_probe(self.stored()))
            });
        if let Err(err) = &probed {
            tracing::warn!(root = %root.display(), error = %err, "pull probe failed");
        }
        probed
    }

    pub fn submodule_outline(&self, root: &Path, parent: &str) -> Result<Vec<Submodule>, GitError> {
        self.on_host_of(root, |root| crate::backend::Request::SubmoduleOutline {
            root,
            parent: parent.to_owned(),
        })
        .unwrap_or_else(|| submodule_outline(root, parent))
    }

    pub(crate) fn pulse_of(&self, root: &Path) -> git_engine::RepoPulse {
        self.on_host_of(root, |root| crate::backend::Request::Pulse { root })
            .map(|pulse| {
                pulse.unwrap_or_else(|err| {
                    tracing::error!(error = ?err, context = "pulse in the agent");
                    git_engine::RepoPulse::default()
                })
            })
            .unwrap_or_else(|| pulse(root))
    }
}
