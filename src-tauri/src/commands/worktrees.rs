//! Linked worktrees: list, add, open, prune, repair, lock, remove (M3).

use super::{blocking, mutating, mutating_titled};
use app_state::{OperationKind, RepoId, RepoSummary, WorktreeScanChunk};
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
    branch: git_engine::WorktreeBranch,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Worktree,
        "add_worktree",
        move || app_state.add_worktree(repo, &path, &branch),
    )
    .await
}

/// Whether text names a commit, with a preview of it; a "no" is data, not an error.
#[tauri::command]
#[specta::specta]
pub async fn check_revision(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
) -> Result<git_engine::RevisionCheck, GitError> {
    let app_state = state.state.clone();
    blocking("check_revision", move || {
        app_state.check_revision(repo, &rev)
    })
    .await
}

/// `None` when `git check-ref-format --branch` accepts the name, else git's complaint.
#[tauri::command]
#[specta::specta]
pub async fn check_branch_name(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
) -> Result<Option<String>, GitError> {
    let app_state = state.state.clone();
    blocking("check_branch_name", move || {
        app_state.check_branch_name(repo, &name)
    })
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

/// What removing a worktree would lose, read as three parallel reads (R-675). Cancellable.
#[tauri::command]
#[specta::specta]
pub async fn scan_worktree_removal(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    on_chunk: tauri::ipc::Channel<WorktreeScanChunk>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    let run = state.state.start_read();
    let cancel = run.token();
    let _ = on_chunk.send(WorktreeScanChunk::Started { id: run.id() });

    let token = cancel.clone();
    let channel = on_chunk.clone();
    let done = blocking("scan_worktree_removal", move || {
        app_state.scan_worktree_removal(
            repo,
            &path,
            move || token.is_cancelled(),
            |chunk| {
                let _ = channel.send(chunk);
            },
        )
    })
    .await;

    drop(run);
    done?;
    let _ = on_chunk.send(WorktreeScanChunk::Done {
        cancelled: cancel.is_cancelled(),
    });
    Ok(())
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

/// `git worktree move <path> <to>`; git's refusal (main, locked, submodules) as it says it.
#[tauri::command]
#[specta::specta]
pub async fn move_worktree(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    to: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Worktree,
        "move_worktree",
        move || app_state.move_worktree(repo, &path, &to),
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

/// Whether a folder git failed to delete is still there, for the follow-up after Remove.
#[tauri::command]
#[specta::specta]
pub async fn worktree_leftover(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
) -> Result<bool, GitError> {
    let app_state = state.state.clone();
    blocking("worktree_leftover", move || {
        app_state.worktree_leftover(repo, &path)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_worktree_leftover(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Worktree,
        "delete_worktree_leftover",
        move || app_state.delete_worktree_leftover(repo, &path),
    )
    .await
}

/// Why a folder cannot take a new worktree (a file, not empty), or `None`.
#[tauri::command]
#[specta::specta]
pub async fn worktree_folder_problem(path: String) -> Result<Option<String>, GitError> {
    blocking("worktree_folder_problem", move || {
        Ok(git_engine::worktree_folder_problem(std::path::Path::new(
            &path,
        )))
    })
    .await
}
