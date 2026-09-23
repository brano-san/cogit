//! Remote ▸ Submodule, Subtree and LFS (#45, #46) and Repository ▸ Settings (#42).

use super::{blocking, mutating};
use app_state::{OperationKind, RepoId};
use git_engine::{GitError, LfsOp, RepoSetting, RepoSettingChange, SubmoduleOp, SubtreeOp};

/// Empty `paths` means every submodule, for Initialize and Synchronize only.
#[tauri::command]
#[specta::specta]
pub async fn submodule_op(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    op: SubmoduleOp,
    paths: Vec<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Submodule,
        "submodule_op",
        move || app_state.submodule_op(repo, op, &paths),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn add_submodule(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    url: String,
    path: String,
    branch: Option<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Submodule,
        "add_submodule",
        move || app_state.add_submodule(repo, &url, &path, branch.as_deref()),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn subtree_op(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    op: SubtreeOp,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Other,
        "subtree_op",
        move || app_state.subtree_op(repo, &op),
    )
    .await
}

/// Walks history for `git-subtree-dir:`; asked when a Subtree dialog opens, not before.
#[tauri::command]
#[specta::specta]
pub async fn subtree_prefixes(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<String>, GitError> {
    let app_state = state.state.clone();
    blocking("subtree_prefixes", move || app_state.subtree_prefixes(repo)).await
}

/// `None` when git has no `lfs` command: only Install stays available then.
#[tauri::command]
#[specta::specta]
pub async fn lfs_version() -> Result<Option<String>, GitError> {
    blocking("lfs_version", || Ok(git_engine::lfs_version())).await
}

#[tauri::command]
#[specta::specta]
pub async fn lfs_op(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    op: LfsOp,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Other,
        "lfs_op",
        move || app_state.lfs_op(repo, &op),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn repo_settings(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<RepoSetting>, GitError> {
    let app_state = state.state.clone();
    blocking("repo_settings", move || app_state.repo_settings(repo)).await
}

#[tauri::command]
#[specta::specta]
pub async fn write_repo_settings(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    changes: Vec<RepoSettingChange>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Other,
        "write_repo_settings",
        move || app_state.write_repo_settings(repo, &changes),
    )
    .await
}
