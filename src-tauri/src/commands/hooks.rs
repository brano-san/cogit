//! Hooks: list, read, write, enable, dry run, the bypass log and user checks (M10).

use super::{blocking, mutating};
use app_state::{OperationKind, RepoId};
use git_engine::GitError;

#[tauri::command]
#[specta::specta]
pub async fn run_check(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    command: String,
) -> Result<git_engine::HookRun, GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Other,
        "run_check",
        move || app_state.run_check(repo, &command),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn list_hooks(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<git_engine::HookOverview, GitError> {
    let app_state = state.state.clone();
    blocking("list_hooks", move || app_state.hooks(repo)).await
}

#[tauri::command]
#[specta::specta]
pub async fn read_hook(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
) -> Result<String, GitError> {
    let app_state = state.state.clone();
    blocking("read_hook", move || app_state.read_hook(repo, &name)).await
}

#[tauri::command]
#[specta::specta]
pub async fn write_hook(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
    body: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Other,
        "write_hook",
        move || app_state.write_hook(repo, &name, &body),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn set_hook_enabled(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
    enabled: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Other,
        "set_hook_enabled",
        move || app_state.set_hook_enabled(repo, &name, enabled),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn use_hooks_path(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Other,
        "use_hooks_path",
        move || app_state.adopt_hooks(repo, &path),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn run_hook(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
) -> Result<git_engine::HookRun, GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Other,
        "run_hook",
        move || app_state.run_hook(repo, &name),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn bypass_log(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<git_engine::Bypass>, GitError> {
    let app_state = state.state.clone();
    blocking("bypass_log", move || app_state.bypass_log(repo)).await
}

#[tauri::command]
#[specta::specta]
pub async fn commit_template(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Option<String>, GitError> {
    let app_state = state.state.clone();
    blocking("commit_template", move || app_state.commit_template(repo)).await
}
