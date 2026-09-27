//! Maintenance, ignore rules, LFS locks, signatures, rerere and range-diff.

use super::{blocking, mutating};
use app_state::{OperationKind, RepoId};
use git_engine::GitError;

/// Asked for by the user after a confirmation; gc and commit-graph only write objects.
#[tauri::command]
#[specta::specta]
pub async fn run_maintenance(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    task: git_engine::MaintenanceTask,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Other,
        "run_maintenance",
        move || app_state.run_maintenance(repo, task),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn add_to_exclude(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    paths: Vec<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Stage,
        "add_to_exclude",
        move || app_state.add_to_exclude(repo, &paths),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn ignore_rules(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    paths: Vec<String>,
) -> Result<Vec<git_engine::IgnoreRule>, GitError> {
    let app_state = state.state.clone();
    blocking("ignore_rules", move || app_state.ignore_rules(repo, &paths)).await
}

/// Asks the LFS server: run on demand, never on a refresh.
#[tauri::command]
#[specta::specta]
pub async fn lfs_locks(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<git_engine::LfsLock>, GitError> {
    let app_state = state.state.clone();
    blocking("lfs_locks", move || app_state.lfs_locks(repo)).await
}

/// The Files LFS column: `.gitattributes` and the local lock cache, no network.
#[tauri::command]
#[specta::specta]
pub async fn lfs_file_states(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    paths: Vec<String>,
) -> Result<Vec<git_engine::LfsFileState>, GitError> {
    let app_state = state.state.clone();
    blocking("lfs_file_states", move || {
        app_state.lfs_file_states(repo, &paths)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn commit_signature(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
) -> Result<git_engine::SignatureCheck, GitError> {
    let app_state = state.state.clone();
    blocking("commit_signature", move || {
        app_state.commit_signature(repo, &rev)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn unportable_paths(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
) -> Result<Vec<String>, GitError> {
    let app_state = state.state.clone();
    blocking("unportable_paths", move || {
        app_state.unportable_paths(repo, &rev)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn rerere_status(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<git_engine::RerereStatus, GitError> {
    let app_state = state.state.clone();
    blocking("rerere_status", move || app_state.rerere_status(repo)).await
}

#[tauri::command]
#[specta::specta]
pub async fn rerere_forget(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    paths: Vec<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Other,
        "rerere_forget",
        move || app_state.rerere_forget(repo, &paths),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn range_diff(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    before: String,
    after: String,
) -> Result<git_engine::GitOutput, GitError> {
    let app_state = state.state.clone();
    blocking("range_diff", move || {
        app_state.range_diff(repo, &before, &after)
    })
    .await
}
