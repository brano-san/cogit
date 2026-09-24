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
