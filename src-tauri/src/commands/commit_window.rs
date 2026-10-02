//! Ctrl+K: the Commit window (F-NNN).

use app_state::RepoId;
use git_engine::GitError;

use super::blocking;

/// Building a window blocks, so it happens on the blocking pool (R-201).
#[tauri::command]
#[specta::specta]
pub async fn open_commit_window(
    app: tauri::AppHandle,
    repo: RepoId,
    root: String,
) -> Result<(), GitError> {
    blocking("open_commit_window", move || {
        crate::child_window::open(
            &app,
            "commit",
            crate::commit_window::url(repo.0, &root),
            crate::commit_window::TITLE.to_owned(),
            crate::commit_window::SHAPE,
        )
        .map_err(|err| GitError::Internal(format!("cannot open the Commit window: {err}")))
    })
    .await
}

/// The last `limit` commits with their full messages, for "Select from Log" and the
/// recent messages of the Commit window.
#[tauri::command]
#[specta::specta]
pub async fn recent_commits(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    limit: u32,
) -> Result<Vec<git_engine::CommitDetails>, GitError> {
    let app_state = state.state.clone();
    blocking("recent_commits", move || {
        app_state.recent_commits(repo, limit)
    })
    .await
}
