//! Remotes and the network: fetch, pull, push with progress, remote URLs and stored tokens (M1).

use super::blocking;
use app_state::{OperationKind, RepoId};
use git_engine::{GitError, NetworkStop};

#[tauri::command]
#[specta::specta]
pub async fn remotes(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<String>, GitError> {
    let app_state = state.state.clone();
    blocking("remotes", move || app_state.remotes(repo)).await
}

/// A network command's own lines go to the page as they arrive, and its phases into the
/// profile once it ends; every command that talks to a remote goes through here.
pub(super) fn with_progress<T>(
    operation: &'static str,
    remote: &str,
    channel: &tauri::ipc::Channel<String>,
    run: impl FnOnce(&mut dyn FnMut(&str)) -> Result<T, GitError>,
) -> Result<T, GitError> {
    let mut timer = git_engine::phases::PhaseTimer::new();
    let result = run(&mut |line: &str| {
        timer.observe(line);
        let _ = channel.send(line.to_owned());
    });
    crate::profile::network(operation, remote, timer, result.is_ok());
    result
}

/// `mutating` for a command that talks to a remote: `cancel_network` can stop it by the
/// id of its queue operation while it runs.
pub(super) async fn networking<T, F>(
    state: &std::sync::Arc<app_state::AppState>,
    repo: RepoId,
    kind: OperationKind,
    label: &'static str,
    work: F,
) -> Result<T, GitError>
where
    T: Send + 'static,
    F: FnOnce(NetworkStop) -> Result<T, GitError> + Send + 'static,
{
    let permit = state.enqueue(repo, kind, kind.title()).await;
    let run = state.network_stop(permit.id());
    let stop = run.token();
    let result = blocking(label, move || work(stop)).await;
    drop(run);
    permit.finish(result.is_ok());
    result
}

/// Stops the fetch, pull or push running as queue operation `operation`. `false` when
/// there is nothing to stop. Off the main thread: stopping waits for `taskkill`.
#[tauri::command]
#[specta::specta]
pub async fn cancel_network(
    state: tauri::State<'_, crate::AppContext>,
    operation: u32,
) -> Result<bool, GitError> {
    let app_state = state.state.clone();
    blocking("cancel_network", move || {
        Ok(app_state.cancel_network(operation))
    })
    .await
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
    networking(
        &state.state,
        repo,
        OperationKind::Fetch,
        "fetch",
        move |stop| {
            with_progress("fetch", &remote, &on_progress, |on_line| {
                app_state.fetch(repo, &remote, &stop, on_line)
            })
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
    networking(
        &state.state,
        repo,
        OperationKind::Pull,
        "pull",
        move |stop| {
            with_progress("pull", &remote, &on_progress, |on_line| {
                app_state.pull(repo, &remote, ff_only, &stop, on_line)
            })
        },
    )
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
    networking(
        &state.state,
        repo,
        OperationKind::Push,
        "push",
        move |stop| {
            with_progress("push", &remote, &on_progress, |on_line| {
                app_state.push(repo, &remote, force, &stop, on_line)
            })
        },
    )
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
    let app_state = state.state.clone();
    blocking("has_token", move || Ok(app_state.has_token(&host))).await
}

#[tauri::command]
#[specta::specta]
pub async fn store_token(
    state: tauri::State<'_, crate::AppContext>,
    host: String,
    token: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("store_token", move || {
        app_state
            .store_token(&host, &token)
            .map_err(|err| GitError::InvalidState(err.to_string()))
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn forget_token(
    state: tauri::State<'_, crate::AppContext>,
    host: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("forget_token", move || {
        app_state
            .forget_token(&host)
            .map_err(|err| GitError::InvalidState(err.to_string()))
    })
    .await
}
