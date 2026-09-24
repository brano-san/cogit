//! Rows of the Repositories list that are not on screen, addressed by folder (R-352).

use super::blocking;
use git_engine::{GitError, Submodule};
use std::path::PathBuf;

/// The light submodule tree of any listed repository, open or closed.
#[tauri::command]
#[specta::specta]
pub async fn submodule_outline(root: String, parent: String) -> Result<Vec<Submodule>, GitError> {
    blocking("submodule_outline", move || {
        app_state::repo_rows::submodule_outline(&PathBuf::from(root), &parent)
    })
    .await
}
