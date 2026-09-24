//! Git Flow: status, init, start and finish (M8).

use super::{blocking, mutating};
use app_state::{OperationKind, RepoId};
use git_engine::GitError;

#[tauri::command]
#[specta::specta]
pub async fn flow_status(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<git_engine::FlowStatus, GitError> {
    let app_state = state.state.clone();
    blocking("flow_status", move || app_state.flow_status(repo)).await
}

#[tauri::command]
#[specta::specta]
pub async fn flow_init(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    config: git_engine::FlowConfig,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Branch,
        "flow_init",
        move || app_state.flow_init(repo, &config),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn flow_start(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    kind: git_engine::FlowKind,
    name: String,
) -> Result<String, GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Branch,
        "flow_start",
        move || app_state.flow_start(repo, kind, &name),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn flow_finish(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    kind: git_engine::FlowKind,
    name: String,
    tag: Option<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Branch,
        "flow_finish",
        move || app_state.flow_finish(repo, kind, &name, tag.as_deref()),
    )
    .await
}
