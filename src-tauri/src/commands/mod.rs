use app_state::{DEFAULT_CHUNK_SIZE, GraphChunk, RepoId, RepoSummary};
use git_engine::GitError;
use serde::Serialize;
use std::path::PathBuf;

/// specta follows serde, so a DTO without `camelCase` reads `undefined` in the UI.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub log_path: String,
    pub debug_build: bool,
}

#[tauri::command]
#[specta::specta]
pub fn app_info(state: tauri::State<'_, crate::AppContext>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        log_path: state.log_path.display().to_string(),
        debug_build: cfg!(debug_assertions),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn open_repository(
    state: tauri::State<'_, crate::AppContext>,
    path: String,
) -> Result<RepoSummary, GitError> {
    let app_state = state.state.clone();
    let path = PathBuf::from(path);

    let started = std::time::Instant::now();
    let summary = tokio::task::spawn_blocking(move || app_state.open_repository(&path))
        .await
        .map_err(|err| GitError::Internal(format!("open_repository task failed: {err}")))??;

    tracing::info!(
        repo = summary.repo.0,
        name = %summary.name,
        branches = summary.branches.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "repository opened"
    );
    Ok(summary)
}

/// A channel rather than a return value (INV-02); dropping it cancels the walk.
#[tauri::command]
#[specta::specta]
pub async fn load_commits(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    on_chunk: tauri::ipc::Channel<GraphChunk>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    let started = std::time::Instant::now();

    let sent = tokio::task::spawn_blocking(move || {
        let mut sent = 0_usize;
        let result = app_state.stream_graph(repo, DEFAULT_CHUNK_SIZE, |chunk| {
            sent += chunk.commits.len();
            on_chunk.send(chunk).is_ok()
        });
        result.map(|()| sent)
    })
    .await
    .map_err(|err| GitError::Internal(format!("load_commits task failed: {err}")))??;

    tracing::info!(
        repo = repo.0,
        commits = sent,
        elapsed_ms = started.elapsed().as_millis(),
        "commit graph streamed"
    );
    Ok(())
}
