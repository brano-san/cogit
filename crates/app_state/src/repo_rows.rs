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

/// Asks the server, writes nothing; a failure is logged and means "unknown" (R-354).
pub fn pull_probe(root: &Path) -> Result<Option<bool>, GitError> {
    let probed = RepoHandle::open_root(root).and_then(|handle| handle.pull_probe());
    if let Err(err) = &probed {
        tracing::warn!(root = %root.display(), error = %err, "pull probe failed");
    }
    probed
}
