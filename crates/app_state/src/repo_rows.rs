//! Rows of the Repositories list not on screen, by folder: a closed one has no `RepoId`.

use git_engine::{GitError, RepoHandle, Submodule};
use std::path::Path;

/// `parent` is a key from the top, `""` for the top (R-352).
pub fn submodule_outline(root: &Path, parent: &str) -> Result<Vec<Submodule>, GitError> {
    let handle = if parent.is_empty() {
        RepoHandle::open(root)?
    } else {
        RepoHandle::open_exact(&root.join(parent))?
    };
    handle.submodule_outline()
}

#[must_use]
pub fn pulse(root: &Path) -> git_engine::RepoPulse {
    git_engine::pulse(root)
}

/// Never prompts, never reaches the Output journal; a failure is logged (R-353).
pub fn background_fetch(root: &Path) -> Result<(), GitError> {
    let started = std::time::Instant::now();
    let fetched = RepoHandle::open(root).and_then(|handle| handle.background_fetch());
    let elapsed_ms = started.elapsed().as_millis();
    match &fetched {
        Ok(()) => tracing::info!(root = %root.display(), elapsed_ms, "background fetch done"),
        Err(err) => {
            tracing::warn!(root = %root.display(), elapsed_ms, error = %err, "background fetch failed");
        }
    }
    fetched
}
