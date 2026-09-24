//! Reads for rows of the Repositories list that are not on screen, open or closed: by
//! folder, never by `RepoId`, because a closed repository has none (R-352).

use git_engine::{GitError, RepoHandle, Submodule};
use std::path::Path;

/// The submodules under `parent` (a key from the top, `""` for the top) of the repository
/// at `root`, from `.gitmodules` and the gitlinks alone.
pub fn submodule_outline(root: &Path, parent: &str) -> Result<Vec<Submodule>, GitError> {
    let handle = if parent.is_empty() {
        RepoHandle::open(root)?
    } else {
        RepoHandle::open_exact(&root.join(parent))?
    };
    handle.submodule_outline()
}

/// The indicators of a row: tracking and changes, read lightly (R-353).
#[must_use]
pub fn pulse(root: &Path) -> git_engine::RepoPulse {
    git_engine::pulse(root)
}

/// A fetch nobody asked for just now: it never prompts, never reaches the Output journal
/// and never notifies. A failure is logged and returned, for the row to say "unknown".
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
