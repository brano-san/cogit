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
use tauri::Manager as _;

/// specta follows serde, so a DTO without `camelCase` reads `undefined` in the UI.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub log_path: String,
    pub debug_build: bool,
}

/// Every blocking command goes through here, so the profile log holds one line per IPC
/// call: what ran, how long it took and whether it worked.
async fn blocking<T, F>(label: &'static str, work: F) -> Result<T, GitError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, GitError> + Send + 'static,
{
    let started = std::time::Instant::now();
    let joined = tokio::task::spawn_blocking(work)
        .await
        .map_err(|err| GitError::Internal(format!("{label} task failed: {err}")))?;
    crate::profile::call(label, started.elapsed(), joined.is_ok());
    joined
}

/// The webview's own clock: how long the user waited between an action and the screen
/// showing its result. Only the backend half is visible from Rust.
#[tauri::command]
#[specta::specta]
pub fn default_keymap() -> Vec<crate::menu::KeyBinding> {
    crate::menu::default_keymap()
}

/// Synchronous: muda has to build the bar on the main thread, and rebuilding is the only
/// way to change an accelerator once an item exists.
#[tauri::command]
#[specta::specta]
pub fn set_keymap(
    app: tauri::AppHandle,
    keymap: tauri::State<'_, crate::menu::Keymap>,
    items: tauri::State<'_, crate::menu::MenuItems<tauri::Wry>>,
    overrides: std::collections::HashMap<String, String>,
) -> Result<(), GitError> {
    crate::menu::rebuild(&app, &keymap, &items, overrides)
        .map_err(|err| GitError::Internal(format!("cannot rebuild the menu: {err}")))
}

#[tauri::command]
#[specta::specta]
pub fn report_timing(label: String, ms: u32, detail: String) {
    crate::profile::ui(&label, u64::from(ms), &detail);
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
    let summary = blocking("open_repository", move || app_state.open_repository(&path)).await?;

    tracing::info!(
        repo = summary.repo.0,
        name = %summary.name,
        branches = summary.branches.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "repository opened"
    );
    Ok(summary)
}

/// A folder can hold hundreds of repositories, so hits stream in as they are found and
/// dropping the channel stops the walk.
#[tauri::command]
#[specta::specta]
pub async fn scan_for_repositories(
    state: tauri::State<'_, crate::AppContext>,
    path: String,
    max_depth: u32,
    on_found: tauri::ipc::Channel<app_state::ScanHit>,
) -> Result<u32, GitError> {
    let app_state = state.state.clone();
    let path = PathBuf::from(path);
    let depth = max_depth.clamp(1, 12) as usize;

    blocking("scan_for_repositories", move || {
        let mut found = 0_u32;
        app_state.scan_for_repositories(&path, depth, |hit| {
            found += 1;
            on_found.send(hit).is_ok()
        });
        Ok(found)
    })
    .await
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

    let sent = blocking("load_commits", move || {
        let mut sent = 0_usize;
        let result = app_state.search_graph(repo, &query, DEFAULT_CHUNK_SIZE, |chunk| {
            sent += chunk.commits.len();
            on_chunk.send(chunk).is_ok()
        });
        result.map(|()| sent)
    })
    .await?;

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
    blocking("commit_details", move || {
        app_state.commit_details(repo, &rev)
    })
    .await
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

    let files = blocking("commit_files", move || app_state.commit_files(repo, &rev)).await?;

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

    let diff = blocking("diff_file", move || {
        app_state.diff_file(repo, &spec, &path, &options)
    })
    .await?;

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
    view: git_engine::WorktreeView,
) -> Result<WorktreeFiles, GitError> {
    let app_state = state.state.clone();
    blocking("worktree_files", move || {
        app_state.worktree_files(repo, view)
    })
    .await
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
            blocking(stringify!($name), move || app_state.$method(repo, &paths)).await
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
    let oid = blocking("commit", move || app_state.commit(repo, &request)).await?;

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
    blocking("checkout", move || app_state.checkout(repo, &target)).await
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
    blocking("create_branch", move || {
        app_state.create_branch(repo, &name, start.as_deref(), switch_to)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn rename_branch(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    from: String,
    to: String,
    force: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("rename_branch", move || {
        app_state.rename_branch(repo, &from, &to, force)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn set_upstream(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    branch: String,
    upstream: Option<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("set_upstream", move || {
        app_state.set_upstream(repo, &branch, upstream.as_deref())
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_remote_branch(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    remote: String,
    branch: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("delete_remote_branch", move || {
        app_state.delete_remote_branch(repo, &remote, &branch)
    })
    .await
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
    blocking("delete_branch", move || {
        app_state.delete_branch(repo, &name, force)
    })
    .await
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
pub async fn run_check(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    command: String,
) -> Result<git_engine::HookRun, GitError> {
    let app_state = state.state.clone();
    blocking("run_check", move || app_state.run_check(repo, &command)).await
}

#[tauri::command]
#[specta::specta]
pub async fn worktrees(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<git_engine::WorktreeEntry>, GitError> {
    let app_state = state.state.clone();
    blocking("worktrees", move || app_state.worktrees(repo)).await
}

#[tauri::command]
#[specta::specta]
pub async fn worktree_holding(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    branch: String,
) -> Result<Option<git_engine::WorktreeEntry>, GitError> {
    let app_state = state.state.clone();
    blocking("worktree_holding", move || {
        app_state.worktree_holding(repo, &branch)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn add_worktree(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    branch: String,
    create: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("add_worktree", move || {
        app_state.add_worktree(repo, &path, &branch, create)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_worktree(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    force: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("remove_worktree", move || {
        app_state.remove_worktree(repo, &path, force)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn prune_worktrees(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("prune_worktrees", move || app_state.prune_worktrees(repo)).await
}

/// The terminals this platform can offer, for the settings dropdown.
#[tauri::command]
#[specta::specta]
pub fn terminal_choices() -> Vec<TerminalChoice> {
    app_state::terminal::choices()
        .into_iter()
        .map(|kind| TerminalChoice {
            id: kind.id().to_owned(),
            label: kind.label().to_owned(),
        })
        .collect()
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TerminalChoice {
    pub id: String,
    pub label: String,
}

/// Spawned with the repository as its working directory, detached from Cogit: closing the
/// client must not close the user's shell.
#[tauri::command]
#[specta::specta]
pub async fn open_in_terminal(path: String, terminal: String) -> Result<(), GitError> {
    let kind = app_state::terminal::Terminal::from_id(&terminal).unwrap_or_default();
    let (program, args) = app_state::terminal::command_for(kind, &path);

    blocking("open_in_terminal", move || {
        std::process::Command::new(&program)
            .args(&args)
            .current_dir(&path)
            .spawn()
            .map(drop)
            .map_err(|err| GitError::Io(format!("cannot start {program}: {err}")))
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn stash_selection(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    paths: Vec<String>,
    message: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("stash_selection", move || {
        app_state.stash_selection(repo, &paths, &message)
    })
    .await
}

/// The three parts of a stash, read without applying it (T5.2).
#[tauri::command]
#[specta::specta]
pub async fn stash_contents(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    index: u32,
) -> Result<git_engine::StashContents, GitError> {
    let app_state = state.state.clone();
    blocking("stash_contents", move || {
        app_state.stash_contents(repo, index)
    })
    .await
}

/// Any entry from the journal, not only the newest (T5.7).
#[tauri::command]
#[specta::specta]
pub async fn undo_entry(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    id: u32,
) -> Result<SafetyEntry, GitError> {
    let app_state = state.state.clone();
    let entry = blocking("undo_entry", move || app_state.undo_entry(repo, id)).await?;

    tracing::info!(repo = repo.0, entry = %entry.description, "operation undone");
    Ok(entry)
}

#[tauri::command]
#[specta::specta]
pub async fn undo_last(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<SafetyEntry, GitError> {
    let app_state = state.state.clone();
    let entry = blocking("undo_last", move || app_state.undo_last(repo)).await?;

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
    blocking("repo_status", move || app_state.repo_status(repo)).await
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
            blocking(stringify!($name), move || app_state.$name(repo)).await
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
    blocking("stashes", move || app_state.stashes(repo)).await
}

#[tauri::command]
#[specta::specta]
pub async fn stash_push(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    options: StashOptions,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("stash_push", move || app_state.stash_push(repo, &options)).await
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
    blocking("stash_apply", move || {
        app_state.stash_apply(repo, index, pop)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn stash_drop(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    index: u32,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("stash_drop", move || app_state.stash_drop(repo, index)).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_tag(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    request: TagRequest,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("create_tag", move || app_state.create_tag(repo, &request)).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_tag(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("delete_tag", move || app_state.delete_tag(repo, &name)).await
}

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
    blocking("fetch", move || {
        let mut timer = git_engine::phases::PhaseTimer::new();
        let named = remote.clone();
        let result = app_state.fetch(repo, &remote, |line| {
            timer.observe(line);
            let _ = on_progress.send(line.to_owned());
        });
        crate::profile::network("fetch", &named, timer, result.is_ok());
        result
    })
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
    blocking("pull", move || {
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
    blocking("push", move || {
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
pub async fn merge(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    options: MergeOptions,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("merge", move || app_state.merge(repo, &options)).await
}

#[tauri::command]
#[specta::specta]
pub async fn rebase(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    options: RebaseOptions,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("rebase", move || app_state.rebase(repo, &options)).await
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
            blocking(stringify!($name), move || app_state.$name(repo, &commits)).await
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
    blocking("reflog", move || app_state.reflog(repo, limit)).await
}

#[tauri::command]
#[specta::specta]
pub async fn lost_commits(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    limit: u32,
) -> Result<Vec<CommitRow>, GitError> {
    let app_state = state.state.clone();
    blocking("lost_commits", move || app_state.lost_commits(repo, limit)).await
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
    blocking("submodules", move || app_state.submodules(repo)).await
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
    blocking("update_submodule", move || {
        app_state.update_submodule(repo, &path, init)
    })
    .await
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
    blocking("stage_selection", move || {
        app_state.stage_selection(repo, &request, reverse)
    })
    .await
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

    let lines = blocking("blame", move || app_state.blame(repo, &path, &rev)).await?;

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
    blocking("remote_url", move || app_state.remote_url(repo, &name)).await
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
    blocking("image_sides", move || {
        app_state.image_sides(repo, &spec, &path)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn conflicted_paths(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<String>, GitError> {
    let app_state = state.state.clone();
    blocking("conflicted_paths", move || app_state.conflicted_paths(repo)).await
}

#[tauri::command]
#[specta::specta]
pub async fn conflict_text(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
) -> Result<git_engine::ConflictText, GitError> {
    let app_state = state.state.clone();
    blocking("conflict_text", move || {
        app_state.conflict_text(repo, &path)
    })
    .await
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
    blocking("resolve_conflict", move || {
        app_state.resolve_conflict(repo, &path, side)
    })
    .await
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
    blocking("resolve_conflict_text", move || {
        app_state.resolve_conflict_text(repo, &path, &text)
    })
    .await
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
    blocking("find_object", move || app_state.find(repo, &query, limit)).await
}

/// Not `async`: touching menu items off the main thread deadlocks on Windows.
#[tauri::command]
#[specta::specta]
pub fn set_menu_state(
    items: tauri::State<'_, crate::menu::MenuItems<tauri::Wry>>,
    disabled: Vec<String>,
    checked: Vec<String>,
) {
    items.apply(&disabled, &checked);
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
    blocking("write_hook", move || {
        app_state.write_hook(repo, &name, &body)
    })
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
    blocking("set_hook_enabled", move || {
        app_state.set_hook_enabled(repo, &name, enabled)
    })
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
    blocking("use_hooks_path", move || app_state.adopt_hooks(repo, &path)).await
}

#[tauri::command]
#[specta::specta]
pub async fn run_hook(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
) -> Result<git_engine::HookRun, GitError> {
    let app_state = state.state.clone();
    blocking("run_hook", move || app_state.run_hook(repo, &name)).await
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
    blocking("rollback_to", move || {
        app_state.rollback_to(repo, &rev, &paths)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn is_published(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
) -> Result<bool, GitError> {
    let app_state = state.state.clone();
    blocking("is_published", move || app_state.is_published(repo, &rev)).await
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
    blocking("split_off", move || {
        app_state.split_off(repo, &rev, &paths, &message, split_first)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn rebase_todo(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    base: String,
) -> Result<Vec<git_engine::TodoEntry>, GitError> {
    let app_state = state.state.clone();
    blocking("rebase_todo", move || app_state.rebase_todo(repo, &base)).await
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
    blocking("interactive_rebase", move || {
        app_state.interactive_rebase(repo, &base, &plan, paused)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn rebase_progress(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Option<git_engine::RebaseProgress>, GitError> {
    let app_state = state.state.clone();
    blocking("rebase_progress", move || app_state.rebase_progress(repo)).await
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
    blocking("overlap_window", move || {
        app_state.overlap_window(repo, &base, &window)
    })
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

#[tauri::command]
#[specta::specta]
pub async fn commit_template(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Option<String>, GitError> {
    let app_state = state.state.clone();
    blocking("commit_template", move || app_state.commit_template(repo)).await
}

#[tauri::command]
#[specta::specta]
pub async fn stage_mode(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    executable: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("stage_mode", move || {
        app_state.stage_mode(repo, &path, executable)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn list_presets(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<app_state::PresetStatus>, GitError> {
    let app_state = state.state.clone();
    blocking("list_presets", move || app_state.presets_for(repo)).await
}

/// Saves the hook as it stands as a preset, so the next repository gets it in one click.
#[tauri::command]
#[specta::specta]
pub async fn export_preset(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    hook: String,
    id: String,
    name: String,
    description: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("export_preset", move || {
        app_state.export_preset(repo, &hook, &id, &name, &description)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_preset(
    state: tauri::State<'_, crate::AppContext>,
    id: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("remove_preset", move || app_state.remove_preset(&id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn install_preset(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    id: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("install_preset", move || {
        app_state.install_preset(repo, &id)
    })
    .await
}

/// The authors on screen. Returns at once with whatever is already cached; anything
/// missing is queued and announced later through `AvatarReady` (M14 T14.2).
#[tauri::command]
#[specta::specta]
pub async fn avatars(
    state: tauri::State<'_, crate::AppContext>,
    authors: Vec<app_state::Author>,
) -> Result<Vec<app_state::AvatarRow>, GitError> {
    let state = state.state.clone();
    blocking("avatars", move || Ok(state.avatars(&authors))).await
}

/// Turning avatars on is also what creates the cache directory: off means no directory,
/// no request and no address leaving the machine (M14 T14.3).
#[tauri::command]
#[specta::specta]
pub async fn set_avatars(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppContext>,
    enabled: bool,
) -> Result<(), GitError> {
    let state = state.state.clone();
    if !enabled {
        state.disable_avatars();
        return Ok(());
    }

    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|err| GitError::Io(format!("no cache directory: {err}")))?
        .join("avatars");

    blocking("set_avatars", move || {
        state
            .enable_avatars(dir)
            .map_err(|err| GitError::Io(err.to_string()))
    })
    .await
}

/// Every file of a commit in one round trip, diffed in parallel (doc/08-diff-engine.md §9).
#[tauri::command]
#[specta::specta]
pub async fn diff_files(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    spec: DiffSpec,
    paths: Vec<String>,
    options: DiffOptions,
    request: u32,
) -> Result<app_state::DiffBatch, GitError> {
    let app_state = state.state.clone();
    let started = std::time::Instant::now();
    let requested = paths.len();

    let batch = blocking("diff_files", move || {
        app_state.diff_files(repo, &spec, &paths, &options, request)
    })
    .await?;

    tracing::debug!(
        repo = repo.0,
        files = requested,
        request,
        superseded = matches!(batch, app_state::DiffBatch::Superseded),
        elapsed_ms = started.elapsed().as_millis(),
        "commit files diffed"
    );
    Ok(batch)
}

/// Throws the selected lines away in the working tree. Destructive and journalled; the
/// view is responsible for confirming it first.
#[tauri::command]
#[specta::specta]
pub async fn discard_selection(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    request: PatchRequest,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("discard_selection", move || {
        app_state.discard_selection(repo, &request)
    })
    .await
}

/// The history of one fragment: every commit that changed it, newest first, with the diff
/// of each edit and the path the file had at the time.
#[tauri::command]
#[specta::specta]
pub async fn investigate(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    from: u32,
    to: u32,
    limit: u32,
) -> Result<Vec<git_engine::InvestigationStep>, GitError> {
    let app_state = state.state.clone();
    let started = std::time::Instant::now();

    let steps = blocking("investigate", move || {
        app_state.investigate(repo, &path, from, to, limit)
    })
    .await?;

    tracing::debug!(
        repo = repo.0,
        steps = steps.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "fragment traced"
    );
    Ok(steps)
}

/// The file as it was before a commit. `None` means there was no such file to open.
#[tauri::command]
#[specta::specta]
pub async fn file_before(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    oid: String,
    path: String,
) -> Result<Option<String>, GitError> {
    let app_state = state.state.clone();
    blocking("file_before", move || {
        app_state.file_before(repo, &oid, &path)
    })
    .await
}

/// A window of its own for one conflicted file, so the merge is not squeezed into a panel.
#[tauri::command]
#[specta::specta]
pub fn open_merge_window(
    app: tauri::AppHandle,
    url: String,
    title: String,
) -> Result<(), GitError> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    let label = format!("merge-{}", app.webview_windows().len());
    WebviewWindowBuilder::new(&app, label, WebviewUrl::App(url.into()))
        .title(title)
        .inner_size(1200.0, 760.0)
        .build()
        .map(drop)
        .map_err(|err| GitError::Internal(format!("cannot open the merge window: {err}")))
}

/// Told by the merge window once it has written the resolution.
#[tauri::command]
#[specta::specta]
pub fn merge_resolved(app: tauri::AppHandle, repo: RepoId, path: String) -> Result<(), GitError> {
    use tauri_specta::Event as _;

    crate::MergeResolved { repo, path }
        .emit(&app)
        .map_err(|err| GitError::Internal(format!("cannot announce the resolution: {err}")))
}

/// The three sides merged into regions, for the four-panel view (doc/08-diff-engine.md §8).
#[tauri::command]
#[specta::specta]
pub async fn merge_preview(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
) -> Result<Vec<diff_engine::Region>, GitError> {
    let app_state = state.state.clone();
    blocking("merge_preview", move || {
        app_state.merge_preview(repo, &path)
    })
    .await
}

/// The shared branches that already hold this commit; empty means it is safe to rewrite.
#[tauri::command]
#[specta::specta]
pub async fn protecting_refs(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
) -> Result<Vec<String>, GitError> {
    let app_state = state.state.clone();
    blocking("protecting_refs", move || {
        app_state.protecting_refs(repo, &rev)
    })
    .await
}
