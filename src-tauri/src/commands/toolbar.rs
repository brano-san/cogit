//! What the toolbar asks for beyond the commands every panel shares.

use super::{blocking, mutating};
use app_state::{OperationKind, RepoId};
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

/// Stash ▸ + Keep Working Tree: the stash is made, the files stay as they are (#29).
#[tauri::command]
#[specta::specta]
pub async fn stash_keeping_worktree(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    message: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Stash,
        "stash_keeping_worktree",
        move || app_state.stash_keeping_worktree(repo, &message),
    )
    .await
}

/// After a pull: local branches merged into HEAD whose upstream the remote deleted.
/// Returns the names that were deleted.
#[tauri::command]
#[specta::specta]
pub async fn delete_merged_branches(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<String>, GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Branch,
        "delete_merged_branches",
        move || app_state.delete_merged_branches(repo),
    )
    .await
}
