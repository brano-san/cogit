//! Repository ▸ Clone… (F-575).

use super::blocking;
use git_engine::{CloneDestination, CloneRequest, GitError, RemoteBranches};

/// `git ls-remote` behind the first page's Next: nothing is written, nobody is asked.
#[tauri::command]
#[specta::specta]
pub async fn remote_branches(
    state: tauri::State<'_, crate::AppContext>,
    source: String,
) -> Result<RemoteBranches, GitError> {
    let app_state = state.state.clone();
    blocking("remote_branches", move || {
        app_state.remote_branches(&source)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn clone_destination(path: String) -> Result<CloneDestination, GitError> {
    blocking("clone_destination", move || {
        Ok(git_engine::clone_destination(std::path::Path::new(&path)))
    })
    .await
}

/// Only a repository URL leaves the clipboard: the rest of it stays out of the page.
#[tauri::command]
#[specta::specta]
pub async fn clipboard_repository_url(app: tauri::AppHandle) -> Result<Option<String>, GitError> {
    use tauri_plugin_clipboard_manager::ClipboardExt as _;
    blocking("clipboard_repository_url", move || {
        Ok(match app.clipboard().read_text() {
            Ok(text) => git_engine::repository_url_in(&text),
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
pub async fn clone_repository(
    state: tauri::State<'_, crate::AppContext>,
    request: CloneRequest,
    on_progress: tauri::ipc::Channel<String>,
) -> Result<String, GitError> {
    let app_state = state.state.clone();
    let permit = state.state.enqueue_clone(&request.target).await;
    let run = state.state.network_stop(permit.id());
    let stop = run.token();
    let host = app_state::host_of(&request.source).unwrap_or_else(|| "local".to_owned());
    let result = blocking("clone_repository", move || {
        super::network::with_progress("clone", &host, &on_progress, |on_line| {
            app_state.clone_repository(&request, &stop, on_line)
        })
    })
    .await;
    drop(run);
    permit.finish(result.is_ok());
    result.map(|root| root.to_string_lossy().into_owned())
}
