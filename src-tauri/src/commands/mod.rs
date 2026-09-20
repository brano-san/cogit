use app_state::{DEFAULT_CHUNK_SIZE, GraphChunk, RepoId, RepoOverview, RepoSummary, SafetyEntry};
use diff_engine::{DiffOptions, FileDiff, PatchRequest};
use git_engine::{BlameLine, CommitRow, ConflictSide, Found, Submodule};
use git_engine::{
    CheckoutTarget, CommitDetails, CommitQuery, CommitRequest, DiffSpec, FileEntry, GitError,
    WorktreeFiles,
};
use git_engine::{
    GitOutput, MergeOptions, RebaseOptions, ReflogEntry, RepoStatus, StashEntry, StashOptions,
    TagRequest,
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
path_command!(add_to_gitignore, add_to_gitignore);
path_command!(delete_untracked, delete_untracked);

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

#[tauri::command]
#[specta::specta]
pub fn safety_log(state: tauri::State<'_, crate::AppContext>) -> Vec<SafetyEntry> {
    state.state.safety_log()
}

#[tauri::command]
#[specta::specta]
pub async fn undo_last(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<SafetyEntry, GitError> {
    let app_state = state.state.clone();
    let entry = tokio::task::spawn_blocking(move || app_state.undo_last(repo))
        .await
        .map_err(|err| GitError::Internal(format!("undo_last task failed: {err}")))??;

    tracing::info!(repo = repo.0, entry = %entry.description, "operation undone");
    Ok(entry)
}

#[tauri::command]
#[specta::specta]
pub async fn repo_status(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<RepoStatus, GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.repo_status(repo))
        .await
        .map_err(|err| GitError::Internal(format!("repo_status task failed: {err}")))?
}

macro_rules! repo_command {
    ($name:ident) => {
        #[tauri::command]
        #[specta::specta]
        pub async fn $name(
            state: tauri::State<'_, crate::AppContext>,
            repo: RepoId,
        ) -> Result<(), GitError> {
            let app_state = state.state.clone();
            tokio::task::spawn_blocking(move || app_state.$name(repo))
                .await
                .map_err(|err| {
                    GitError::Internal(format!(concat!(stringify!($name), " task failed: {}"), err))
                })?
        }
    };
}

repo_command!(abort_operation);
repo_command!(continue_operation);
repo_command!(skip_operation);

#[tauri::command]
#[specta::specta]
pub async fn stashes(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<StashEntry>, GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.stashes(repo))
        .await
        .map_err(|err| GitError::Internal(format!("stashes task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn stash_push(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    options: StashOptions,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.stash_push(repo, &options))
        .await
        .map_err(|err| GitError::Internal(format!("stash_push task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn stash_apply(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    index: u32,
    pop: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.stash_apply(repo, index, pop))
        .await
        .map_err(|err| GitError::Internal(format!("stash_apply task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn stash_drop(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    index: u32,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.stash_drop(repo, index))
        .await
        .map_err(|err| GitError::Internal(format!("stash_drop task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn create_tag(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    request: TagRequest,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.create_tag(repo, &request))
        .await
        .map_err(|err| GitError::Internal(format!("create_tag task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn delete_tag(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.delete_tag(repo, &name))
        .await
        .map_err(|err| GitError::Internal(format!("delete_tag task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn remotes(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<String>, GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.remotes(repo))
        .await
        .map_err(|err| GitError::Internal(format!("remotes task failed: {err}")))?
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
    tokio::task::spawn_blocking(move || {
        app_state.fetch(repo, &remote, |line| {
            let _ = on_progress.send(line.to_owned());
        })
    })
    .await
    .map_err(|err| GitError::Internal(format!("fetch task failed: {err}")))?
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
    tokio::task::spawn_blocking(move || {
        app_state.pull(repo, &remote, ff_only, |line| {
            let _ = on_progress.send(line.to_owned());
        })
    })
    .await
    .map_err(|err| GitError::Internal(format!("pull task failed: {err}")))?
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
    tokio::task::spawn_blocking(move || {
        app_state.push(repo, &remote, force, |line| {
            let _ = on_progress.send(line.to_owned());
        })
    })
    .await
    .map_err(|err| GitError::Internal(format!("push task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn merge(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    options: MergeOptions,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.merge(repo, &options))
        .await
        .map_err(|err| GitError::Internal(format!("merge task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn rebase(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    options: RebaseOptions,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.rebase(repo, &options))
        .await
        .map_err(|err| GitError::Internal(format!("rebase task failed: {err}")))?
}

macro_rules! replay_command {
    ($name:ident) => {
        #[tauri::command]
        #[specta::specta]
        pub async fn $name(
            state: tauri::State<'_, crate::AppContext>,
            repo: RepoId,
            commits: Vec<String>,
        ) -> Result<(), GitError> {
            let app_state = state.state.clone();
            tokio::task::spawn_blocking(move || app_state.$name(repo, &commits))
                .await
                .map_err(|err| {
                    GitError::Internal(format!(concat!(stringify!($name), " task failed: {}"), err))
                })?
        }
    };
}

replay_command!(cherry_pick);
replay_command!(revert);

#[tauri::command]
#[specta::specta]
pub async fn reflog(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    limit: u32,
) -> Result<Vec<ReflogEntry>, GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.reflog(repo, limit))
        .await
        .map_err(|err| GitError::Internal(format!("reflog task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn lost_commits(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    limit: u32,
) -> Result<Vec<CommitRow>, GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.lost_commits(repo, limit))
        .await
        .map_err(|err| GitError::Internal(format!("lost_commits task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub fn repositories(state: tauri::State<'_, crate::AppContext>) -> Vec<RepoOverview> {
    state.state.overviews()
}

#[tauri::command]
#[specta::specta]
pub fn close_repository(state: tauri::State<'_, crate::AppContext>, repo: RepoId) -> bool {
    state.state.close_repository(repo)
}

#[tauri::command]
#[specta::specta]
pub async fn submodules(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<Submodule>, GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.submodules(repo))
        .await
        .map_err(|err| GitError::Internal(format!("submodules task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn update_submodule(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    init: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.update_submodule(repo, &path, init))
        .await
        .map_err(|err| GitError::Internal(format!("update_submodule task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn stage_selection(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    request: PatchRequest,
    reverse: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.stage_selection(repo, &request, reverse))
        .await
        .map_err(|err| GitError::Internal(format!("stage_selection task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn blame(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    rev: String,
) -> Result<Vec<BlameLine>, GitError> {
    let app_state = state.state.clone();
    let started = std::time::Instant::now();

    let lines = tokio::task::spawn_blocking(move || app_state.blame(repo, &path, &rev))
        .await
        .map_err(|err| GitError::Internal(format!("blame task failed: {err}")))??;

    tracing::debug!(
        repo = repo.0,
        lines = lines.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "blame computed"
    );
    Ok(lines)
}

#[tauri::command]
#[specta::specta]
pub async fn remote_url(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
) -> Result<Option<String>, GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.remote_url(repo, &name))
        .await
        .map_err(|err| GitError::Internal(format!("remote_url task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn image_sides(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    spec: DiffSpec,
    path: String,
) -> Result<(Option<String>, Option<String>), GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.image_sides(repo, &spec, &path))
        .await
        .map_err(|err| GitError::Internal(format!("image_sides task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn conflicted_paths(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<String>, GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.conflicted_paths(repo))
        .await
        .map_err(|err| GitError::Internal(format!("conflicted_paths task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn conflict_text(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
) -> Result<git_engine::ConflictText, GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.conflict_text(repo, &path))
        .await
        .map_err(|err| GitError::Internal(format!("conflict_text task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn resolve_conflict(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    side: ConflictSide,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.resolve_conflict(repo, &path, side))
        .await
        .map_err(|err| GitError::Internal(format!("resolve_conflict task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn resolve_conflict_text(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    text: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.resolve_conflict_text(repo, &path, &text))
        .await
        .map_err(|err| GitError::Internal(format!("resolve_conflict_text task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn find_object(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    query: String,
    limit: u32,
) -> Result<Vec<Found>, GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.find(repo, &query, limit))
        .await
        .map_err(|err| GitError::Internal(format!("find_object task failed: {err}")))?
}

/// Not `async`: touching menu items off the main thread deadlocks on Windows.
#[tauri::command]
#[specta::specta]
pub fn set_menu_state(
    items: tauri::State<'_, crate::menu::MenuItems<tauri::Wry>>,
    disabled: Vec<String>,
) {
    items.set_enabled(&disabled);
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

#[tauri::command]
#[specta::specta]
pub async fn list_hooks(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<git_engine::HookOverview, GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.hooks(repo))
        .await
        .map_err(|err| GitError::Internal(format!("list_hooks task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn read_hook(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
) -> Result<String, GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.read_hook(repo, &name))
        .await
        .map_err(|err| GitError::Internal(format!("read_hook task failed: {err}")))?
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
    tokio::task::spawn_blocking(move || app_state.write_hook(repo, &name, &body))
        .await
        .map_err(|err| GitError::Internal(format!("write_hook task failed: {err}")))?
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
    tokio::task::spawn_blocking(move || app_state.set_hook_enabled(repo, &name, enabled))
        .await
        .map_err(|err| GitError::Internal(format!("set_hook_enabled task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn use_hooks_path(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.use_hooks_path(repo, &path))
        .await
        .map_err(|err| GitError::Internal(format!("use_hooks_path task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn run_hook(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
) -> Result<git_engine::HookRun, GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.run_hook(repo, &name))
        .await
        .map_err(|err| GitError::Internal(format!("run_hook task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn rollback_to(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
    paths: Vec<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.rollback_to(repo, &rev, &paths))
        .await
        .map_err(|err| GitError::Internal(format!("rollback_to task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn is_published(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
) -> Result<bool, GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.is_published(repo, &rev))
        .await
        .map_err(|err| GitError::Internal(format!("is_published task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn split_off(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
    paths: Vec<String>,
    message: String,
    split_first: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || {
        app_state.split_off(repo, &rev, &paths, &message, split_first)
    })
    .await
    .map_err(|err| GitError::Internal(format!("split_off task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn rebase_todo(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    base: String,
) -> Result<Vec<git_engine::TodoEntry>, GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.rebase_todo(repo, &base))
        .await
        .map_err(|err| GitError::Internal(format!("rebase_todo task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn interactive_rebase(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    base: String,
    plan: Vec<git_engine::TodoEntry>,
    paused: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.interactive_rebase(repo, &base, &plan, paused))
        .await
        .map_err(|err| GitError::Internal(format!("interactive_rebase task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn rebase_progress(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Option<git_engine::RebaseProgress>, GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.rebase_progress(repo))
        .await
        .map_err(|err| GitError::Internal(format!("rebase_progress task failed: {err}")))?
}

/// `spawn_blocking` matters here: the engine fans out with rayon, which must never run on
/// a Tokio worker (INV-01).
#[tauri::command]
#[specta::specta]
pub async fn overlap_window(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    base: String,
    window: Vec<String>,
) -> Result<Vec<git_engine::OverlapRow>, GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.overlap_window(repo, &base, &window))
        .await
        .map_err(|err| GitError::Internal(format!("overlap_window task failed: {err}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn bypass_log(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<git_engine::Bypass>, GitError> {
    let app_state = state.state.clone();
    tokio::task::spawn_blocking(move || app_state.bypass_log(repo))
        .await
        .map_err(|err| GitError::Internal(format!("bypass_log task failed: {err}")))?
}

/// Not `async`: menu APIs must run on the main thread on Windows.
#[tauri::command]
#[specta::specta]
pub fn popup_context_menu(
    window: tauri::Window,
    held: tauri::State<'_, crate::menu::ContextMenu<tauri::Wry>>,
    items: Vec<crate::menu::ContextItem>,
    x: f64,
    y: f64,
) -> Result<(), GitError> {
    crate::menu::popup(&window, &held, &items, x, y)
        .map_err(|err| GitError::Internal(format!("cannot open the context menu: {err}")))
}

/// Not `async`: creating a window has to happen on the main thread. The parameters ride in
/// the URL so the window rebuilds itself after a webview reload (T2.5).
#[tauri::command]
#[specta::specta]
pub fn open_compare_window(
    app: tauri::AppHandle,
    url: String,
    title: String,
) -> Result<(), GitError> {
    use tauri::{Manager as _, WebviewUrl, WebviewWindowBuilder};

    let label = format!("compare-{}", app.webview_windows().len());
    WebviewWindowBuilder::new(&app, label, WebviewUrl::App(url.into()))
        .title(title)
        .inner_size(1000.0, 720.0)
        .build()
        .map(drop)
        .map_err(|err| GitError::Internal(format!("cannot open the compare window: {err}")))
}
