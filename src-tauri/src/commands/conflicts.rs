//! Conflicts and the 3-way merge window (M7).

use super::{blocking, mutating};
use app_state::{OperationKind, RepoId};
use git_engine::{ConflictSide, GitError};

#[tauri::command]
#[specta::specta]
pub async fn conflicted_paths(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<String>, GitError> {
    let app_state = state.state.clone();
    blocking("conflicted_paths", move || app_state.conflicted_paths(repo)).await
}

#[tauri::command]
#[specta::specta]
pub async fn conflict_text(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
) -> Result<git_engine::ConflictText, GitError> {
    let app_state = state.state.clone();
    blocking("conflict_text", move || {
        app_state.conflict_text(repo, &path)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn resolve_conflict(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    side: ConflictSide,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Merge,
        "resolve_conflict",
        move || app_state.resolve_conflict(repo, &path, side),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn resolve_conflict_text(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    text: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Merge,
        "resolve_conflict_text",
        move || app_state.resolve_conflict_text(repo, &path, &text),
    )
    .await
}

/// A window of its own for one conflicted file, so the merge is not squeezed into a panel.
#[tauri::command]
#[specta::specta]
pub async fn open_merge_window(
    app: tauri::AppHandle,
    url: String,
    title: String,
) -> Result<(), GitError> {
    blocking("open_merge_window", move || {
        crate::child_window::open(
            &app,
            "merge",
            url,
            title,
            crate::child_window::Shape {
                width: 1200.0,
                height: 760.0,
                min_width: 800.0,
                min_height: 500.0,
            },
        )
        .map_err(|err| GitError::Internal(format!("cannot open the merge window: {err}")))
    })
    .await
}

/// Told by the merge window once it has written the resolution.
#[tauri::command]
#[specta::specta]
pub fn merge_resolved(app: tauri::AppHandle, repo: RepoId, path: String) -> Result<(), GitError> {
    use tauri_specta::Event as _;

    crate::MergeResolved { repo, path }
        .emit(&app)
        .map_err(|err| GitError::Internal(format!("cannot announce the resolution: {err}")))
}

/// The three sides merged into regions, for the four-panel view (doc/08-diff-engine.md §8).
#[tauri::command]
#[specta::specta]
pub async fn merge_preview(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
) -> Result<Vec<diff_engine::Region>, GitError> {
    let app_state = state.state.clone();
    blocking("merge_preview", move || {
        app_state.merge_preview(repo, &path)
    })
    .await
}
