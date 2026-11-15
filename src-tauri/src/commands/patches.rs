//! Patches: save and copy commits or working-tree changes as patches, apply one (F-948).

use super::{blocking, mutating};
use app_state::{OperationKind, RepoId};
use git_engine::{GitError, PatchMode, PatchPreview};
use std::path::Path;

/// `target`: a folder for one file per commit, or the one file when `one_file`.
#[tauri::command]
#[specta::specta]
pub async fn save_commit_patches(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    commits: Vec<String>,
    target: String,
    one_file: bool,
) -> Result<Vec<String>, GitError> {
    let app_state = state.state.clone();
    blocking("save_commit_patches", move || {
        app_state.save_commit_patches(repo, &commits, Path::new(&target), one_file)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn commit_patch_text(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    commits: Vec<String>,
) -> Result<String, GitError> {
    let app_state = state.state.clone();
    blocking("commit_patch_text", move || {
        app_state.commit_patch_text(repo, &commits)
    })
    .await
}

/// `staged`: the Staged list (`diff --cached`), else the Unstaged one.
#[tauri::command]
#[specta::specta]
pub async fn save_worktree_patch(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    paths: Vec<String>,
    staged: bool,
    target: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("save_worktree_patch", move || {
        app_state.save_worktree_patch(repo, &paths, staged, Path::new(&target))
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn worktree_patch_text(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    paths: Vec<String>,
    staged: bool,
) -> Result<String, GitError> {
    let app_state = state.state.clone();
    blocking("worktree_patch_text", move || {
        app_state.worktree_patch_text(repo, &paths, staged)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn read_patch_file(path: String) -> Result<String, GitError> {
    blocking("read_patch_file", move || {
        app_state::read_patch_file(Path::new(&path))
    })
    .await
}

/// Only a patch leaves the clipboard: the rest of it stays out of the page (R-563).
#[tauri::command]
#[specta::specta]
pub async fn clipboard_patch(app: tauri::AppHandle) -> Result<Option<String>, GitError> {
    use tauri_plugin_clipboard_manager::ClipboardExt as _;
    blocking("clipboard_patch", move || {
        Ok(match app.clipboard().read_text() {
            Ok(text) if git_engine::looks_like_patch(&text) => {
                Some(git_engine::pasted_patch(&text))
            }
            Ok(_) => None,
            Err(err) => {
                tracing::debug!(error = ?err, context = "the clipboard holds no text");
                None
            }
        })
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn patch_preview(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    text: String,
    mode: PatchMode,
) -> Result<PatchPreview, GitError> {
    let app_state = state.state.clone();
    blocking("patch_preview", move || {
        app_state.patch_preview(repo, &text, mode)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn apply_patch(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    text: String,
    mode: PatchMode,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Other,
        "apply_patch",
        move || app_state.apply_user_patch(repo, &text, mode),
    )
    .await
}
