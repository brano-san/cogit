//! What the toolbar asks for beyond the commands every panel shares.

use super::blocking;
use app_state::RepoId;
use git_engine::GitError;

/// Merge is offered only for a commit HEAD does not already contain (task #31).
#[tauri::command]
#[specta::specta]
pub async fn is_merged_into_head(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
) -> Result<bool, GitError> {
    let app_state = state.state.clone();
    blocking("is_merged_into_head", move || {
        app_state.is_merged_into_head(repo, &rev)
    })
    .await
}
