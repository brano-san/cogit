//! A remote's own menu in Branches: Properties, Rename, Delete, Fetch More, Set Depth (M5).

use super::{blocking, mutating_titled};
use app_state::{OperationKind, RepoId};
use git_engine::{GitError, RemoteInfo};

#[tauri::command]
#[specta::specta]
pub async fn remote_info(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
) -> Result<RemoteInfo, GitError> {
    let app_state = state.state.clone();
    blocking("remote_info", move || app_state.remote_info(repo, &name)).await
}

#[tauri::command]
#[specta::specta]
pub async fn rename_remote(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    from: String,
    to: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating_titled(
        &state.state,
        repo,
        OperationKind::Other,
        "Renaming the remote",
        "rename_remote",
        move || app_state.rename_remote(repo, &from, &to),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_remote(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating_titled(
        &state.state,
        repo,
        OperationKind::Other,
        "Deleting the remote",
        "remove_remote",
        move || app_state.remove_remote(repo, &name),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn set_remote_properties(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
    url: String,
    background_fetch: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating_titled(
        &state.state,
        repo,
        OperationKind::Other,
        "Saving the remote",
        "set_remote_properties",
        move || app_state.set_remote_properties(repo, &name, &url, background_fetch),
    )
    .await
}

/// `false`: nothing new came from the remote.
#[tauri::command]
#[specta::specta]
pub async fn fetch_more(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    remote: String,
    on_progress: tauri::ipc::Channel<String>,
) -> Result<bool, GitError> {
    let app_state = state.state.clone();
    super::network::networking(
        &state.state,
        repo,
        OperationKind::Fetch,
        "fetch_more",
        move |stop| {
            super::network::with_progress("fetch", &remote, &on_progress, |on_line| {
                app_state.fetch_more(repo, &remote, &stop, on_line)
            })
        },
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn fetch_depth(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    remote: String,
    depth: u32,
    on_progress: tauri::ipc::Channel<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    super::network::networking(
        &state.state,
        repo,
        OperationKind::Fetch,
        "fetch_depth",
        move |stop| {
            super::network::with_progress("fetch", &remote, &on_progress, |on_line| {
                app_state.fetch_depth(repo, &remote, depth, &stop, on_line)
            })
        },
    )
    .await
}
