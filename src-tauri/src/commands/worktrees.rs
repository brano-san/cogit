//! Linked worktrees: list, add, open, prune, repair, lock, remove (M3).

use super::{blocking, mutating, mutating_titled};
use app_state::{OperationKind, RepoId, RepoSummary};
use git_engine::GitError;

#[tauri::command]
#[specta::specta]
pub async fn worktrees(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<git_engine::WorktreeEntry>, GitError> {
    let app_state = state.state.clone();
    let found = blocking("worktrees", move || app_state.worktrees(repo)).await?;
    tracing::info!(repo = repo.0, worktrees = found.len(), "worktrees listed");
    Ok(found)
}

#[tauri::command]
#[specta::specta]
pub async fn worktree_holding(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    branch: String,
) -> Result<Option<git_engine::WorktreeEntry>, GitError> {
    let app_state = state.state.clone();
    blocking("worktree_holding", move || {
        app_state.worktree_holding(repo, &branch)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn add_worktree(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    branch: String,
    create: bool,
    base: Option<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Worktree,
        "add_worktree",
        move || app_state.add_worktree(repo, &path, &branch, create, base.as_deref()),
    )
    .await
}

/// A worktree in the panels, not in the Repositories list (R-184).
#[tauri::command]
#[specta::specta]
pub async fn open_worktree(
    state: tauri::State<'_, crate::AppContext>,
    owner: RepoId,
    path: String,
) -> Result<RepoSummary, GitError> {
    let app_state = state.state.clone();
    blocking("open_worktree", move || {
        app_state.open_worktree(owner, &path)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn worktree_changes(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
) -> Result<Vec<git_engine::FileEntry>, GitError> {
    let app_state = state.state.clone();
    blocking("worktree_changes", move || {
        app_state.worktree_changes(repo, &path)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn prune_worktree(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Worktree,
        "prune_worktree",
        move || app_state.prune_worktree(repo, &path),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn repair_worktree(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    let name = path.rsplit('/').next().unwrap_or(&path).to_owned();
    mutating_titled(
        &state.state,
        repo,
        OperationKind::Worktree,
        &format!("Repairing worktree {name}"),
        "repair_worktree",
        move || app_state.repair_worktree(repo, &path),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn lock_worktree(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    reason: Option<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Worktree,
        "lock_worktree",
        move || app_state.lock_worktree(repo, &path, reason.as_deref()),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn unlock_worktree(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Worktree,
        "unlock_worktree",
        move || app_state.unlock_worktree(repo, &path),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_worktree(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    force: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Worktree,
        "remove_worktree",
        move || app_state.remove_worktree(repo, &path, force),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn prune_worktrees(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Worktree,
        "prune_worktrees",
        move || app_state.prune_worktrees(repo),
    )
    .await
}
