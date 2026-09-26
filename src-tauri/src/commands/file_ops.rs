//! The file context menus of the Files panel (#40, #41) beyond staging.

use app_state::{OperationKind, RepoId};
use git_engine::{GitError, IndexEditorSides, IndexFlag};

use super::{blocking, mutating};

/// `git rm --cached` or, with `delete_local`, `git rm`.
#[tauri::command]
#[specta::specta]
pub async fn remove_from_repository(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    paths: Vec<String>,
    delete_local: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Discard,
        "remove_from_repository",
        move || app_state.remove_from_repository(repo, &paths, delete_local),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn move_path(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    from: String,
    to: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Stage,
        "move_path",
        move || app_state.move_path(repo, &from, &to),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn set_index_flag(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    paths: Vec<String>,
    flag: IndexFlag,
    on: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Stage,
        "set_index_flag",
        move || app_state.set_index_flag(repo, &paths, flag, on),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn index_editor_sides(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
) -> Result<IndexEditorSides, GitError> {
    let app_state = state.state.clone();
    blocking("index_editor_sides", move || {
        app_state.index_editor_sides(repo, &path)
    })
    .await
}

/// A side sent as `null` was not edited and stays as it is.
#[tauri::command]
#[specta::specta]
pub async fn write_index_editor(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    index: Option<String>,
    worktree: Option<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Stage,
        "write_index_editor",
        move || app_state.write_index_editor(repo, &path, index.as_deref(), worktree.as_deref()),
    )
    .await
}

/// `target` is an absolute path the user picked in the save dialog.
#[tauri::command]
#[specta::specta]
pub async fn save_blob(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
    path: String,
    target: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("save_blob", move || {
        app_state.save_blob(repo, &rev, &path, std::path::Path::new(&target))
    })
    .await
}

/// A read-only copy of the version in `rev`, opened in the application paired with it.
#[tauri::command]
#[specta::specta]
pub async fn open_read_only(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
    path: String,
) -> Result<String, GitError> {
    let app_state = state.state.clone();
    blocking("open_read_only", move || {
        app_state.open_read_only(repo, &rev, &path)
    })
    .await
}

/// One file's change from `rev`, applied forward (Cherry-Pick) or backward (Revert).
#[tauri::command]
#[specta::specta]
pub async fn apply_commit_file(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
    path: String,
    old_path: Option<String>,
    reverse: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Other,
        "apply_commit_file",
        move || app_state.apply_commit_file(repo, &rev, &path, old_path.as_deref(), reverse),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn present_on_disk(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    paths: Vec<String>,
) -> Result<Vec<String>, GitError> {
    let app_state = state.state.clone();
    blocking("present_on_disk", move || {
        app_state.present_on_disk(repo, &paths)
    })
    .await
}
