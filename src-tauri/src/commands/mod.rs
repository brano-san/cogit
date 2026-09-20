use app_state::{DEFAULT_CHUNK_SIZE, GraphChunk, RepoId, RepoSummary};
use diff_engine::{DiffOptions, FileDiff};
use git_engine::GitOutput;
use git_engine::{
    CheckoutTarget, CommitDetails, CommitQuery, CommitRequest, DiffSpec, FileEntry, GitError,
    WorktreeFiles,
};
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
    query: CommitQuery,
    on_chunk: tauri::ipc::Channel<GraphChunk>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    let started = std::time::Instant::now();

    let sent = tokio::task::spawn_blocking(move || {
        let mut sent = 0_usize;
        let result = app_state.search_graph(repo, &query, DEFAULT_CHUNK_SIZE, |chunk| {
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

#[tauri::command]
#[specta::specta]
pub async fn commit_details(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
) -> Result<CommitDetails, GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.commit_details(repo, &rev))
        .await
        .map_err(|err| GitError::Internal(format!("commit_details task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn commit_files(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
) -> Result<Vec<FileEntry>, GitError> {
    let app_state = state.state.clone();
    let started = std::time::Instant::now();

    let files = tokio::task::spawn_blocking(move || app_state.commit_files(repo, &rev))
        .await
        .map_err(|err| GitError::Internal(format!("commit_files task failed: {err}")))??;

    tracing::debug!(
        repo = repo.0,
        files = files.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "commit files listed"
    );
    Ok(files)
}

#[tauri::command]
#[specta::specta]
pub async fn diff_file(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    spec: DiffSpec,
    path: String,
    options: DiffOptions,
) -> Result<FileDiff, GitError> {
    let app_state = state.state.clone();
    let started = std::time::Instant::now();
    let logged = path.clone();

    let diff =
        tokio::task::spawn_blocking(move || app_state.diff_file(repo, &spec, &path, &options))
            .await
            .map_err(|err| GitError::Internal(format!("diff_file task failed: {err}")))??;

    tracing::debug!(
        repo = repo.0,
        path = %logged,
        elapsed_ms = started.elapsed().as_millis(),
        "file diff computed"
    );
    Ok(diff)
}

#[tauri::command]
#[specta::specta]
pub async fn worktree_files(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<WorktreeFiles, GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.worktree_files(repo))
        .await
        .map_err(|err| GitError::Internal(format!("worktree_files task failed: {err}")))?
}

macro_rules! path_command {
    ($name:ident, $method:ident) => {
        #[tauri::command]
        #[specta::specta]
        pub async fn $name(
            state: tauri::State<'_, crate::AppContext>,
            repo: RepoId,
            paths: Vec<String>,
        ) -> Result<(), GitError> {
            let app_state = state.state.clone();
            tokio::task::spawn_blocking(move || app_state.$method(repo, &paths))
                .await
                .map_err(|err| {
                    GitError::Internal(format!(concat!(stringify!($name), " task failed: {}"), err))
                })?
        }
    };
}

path_command!(stage_paths, stage_paths);
path_command!(unstage_paths, unstage_paths);
path_command!(discard_paths, discard_paths);

#[tauri::command]
#[specta::specta]
pub async fn commit(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    request: CommitRequest,
) -> Result<String, GitError> {
    let app_state = state.state.clone();
    let oid = tokio::task::spawn_blocking(move || app_state.commit(repo, &request))
        .await
        .map_err(|err| GitError::Internal(format!("commit task failed: {err}")))??;

    tracing::info!(repo = repo.0, oid = %oid, "commit created");
    Ok(oid)
}

#[tauri::command]
#[specta::specta]
pub async fn checkout(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    target: CheckoutTarget,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.checkout(repo, &target))
        .await
        .map_err(|err| GitError::Internal(format!("checkout task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn create_branch(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
    start: Option<String>,
    // Not `switch`: specta puts the parameter name straight into the generated TypeScript,
    // where a reserved word is a syntax error.
    switch_to: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || {
        app_state.create_branch(repo, &name, start.as_deref(), switch_to)
    })
    .await
    .map_err(|err| GitError::Internal(format!("create_branch task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn delete_branch(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
    force: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.delete_branch(repo, &name, force))
        .await
        .map_err(|err| GitError::Internal(format!("delete_branch task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub fn command_log(state: tauri::State<'_, crate::AppContext>) -> Vec<GitOutput> {
    state.state.command_log()
}

#[tauri::command]
#[specta::specta]
pub fn command_problems(state: tauri::State<'_, crate::AppContext>) -> u32 {
    state.state.command_problems()
}

#[tauri::command]
#[specta::specta]
pub fn clear_command_log(state: tauri::State<'_, crate::AppContext>) {
    state.state.clear_command_log();
}
