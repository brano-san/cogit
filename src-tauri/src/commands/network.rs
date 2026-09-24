//! Remotes and the network: fetch, pull, push with progress, remote URLs and stored tokens (M1).

use super::{blocking, mutating};
use app_state::{OperationKind, RepoId};
use git_engine::GitError;

#[tauri::command]
#[specta::specta]
pub async fn remotes(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<String>, GitError> {
    let app_state = state.state.clone();
    blocking("remotes", move || app_state.remotes(repo)).await
}

#[tauri::command]
#[specta::specta]
pub async fn fetch(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    remote: String,
    on_progress: tauri::ipc::Channel<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Fetch,
        "fetch",
        move || {
            let mut timer = git_engine::phases::PhaseTimer::new();
            let named = remote.clone();
            let result = app_state.fetch(repo, &remote, |line| {
                timer.observe(line);
                let _ = on_progress.send(line.to_owned());
            });
            crate::profile::network("fetch", &named, timer, result.is_ok());
            result
        },
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn pull(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    remote: String,
    ff_only: bool,
    on_progress: tauri::ipc::Channel<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(&state.state, repo, OperationKind::Pull, "pull", move || {
        let mut timer = git_engine::phases::PhaseTimer::new();
        let named = remote.clone();
        let result = app_state.pull(repo, &remote, ff_only, |line| {
            timer.observe(line);
            let _ = on_progress.send(line.to_owned());
        });
        crate::profile::network("pull", &named, timer, result.is_ok());
        result
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn push(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    remote: String,
    force: bool,
    on_progress: tauri::ipc::Channel<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(&state.state, repo, OperationKind::Push, "push", move || {
        let mut timer = git_engine::phases::PhaseTimer::new();
        let named = remote.clone();
        let result = app_state.push(repo, &remote, force, |line| {
            timer.observe(line);
            let _ = on_progress.send(line.to_owned());
        });
        crate::profile::network("push", &named, timer, result.is_ok());
        result
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn remote_url(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
) -> Result<Option<String>, GitError> {
    let app_state = state.state.clone();
    blocking("remote_url", move || app_state.remote_url(repo, &name)).await
}

/// Reports only whether a token exists. Reading one back would put it in the webview,
/// where every dependency could see it.
#[tauri::command]
#[specta::specta]
pub async fn has_token(
    state: tauri::State<'_, crate::AppContext>,
    host: String,
) -> Result<bool, GitError> {
    Ok(state.state.has_token(&host))
}

#[tauri::command]
#[specta::specta]
pub async fn store_token(
    state: tauri::State<'_, crate::AppContext>,
    host: String,
    token: String,
) -> Result<(), GitError> {
    state
        .state
        .store_token(&host, &token)
        .map_err(|err| GitError::InvalidState(err.to_string()))
}

#[tauri::command]
#[specta::specta]
pub async fn forget_token(
    state: tauri::State<'_, crate::AppContext>,
    host: String,
) -> Result<(), GitError> {
    state
        .state
        .forget_token(&host)
        .map_err(|err| GitError::InvalidState(err.to_string()))
}
