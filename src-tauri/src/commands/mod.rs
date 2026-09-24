use app_state::{
    DEFAULT_CHUNK_SIZE, GraphProgress, OperationKind, RepoId, RepoOverview, RepoSummary,
    SafetyEntry,
};
use diff_engine::{DiffOptions, FileDiff, PatchRequest};
use git_engine::{BlameLine, CommitRow, Found, Submodule};
use git_engine::{
    CommitDetails, CommitQuery, CommitRequest, DiffSpec, FileEntry, GitError, WorktreeFiles,
};
use git_engine::{GitOutput, MergeOptions, RebaseOptions, RepoStatus};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod avatars;
pub mod branches;
pub mod conflicts;
pub mod desktop;
pub mod file_ops;
pub mod flow;
pub mod hooks;
pub mod investigate;
pub mod network;
pub mod presets;
pub mod ref_ops;
pub mod remote_ops;
pub mod stash;
pub mod toolbar;
pub mod worktrees;

/// specta follows serde, so a DTO without `camelCase` reads `undefined` in the UI.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub debug_build: bool,
    /// What a bug report needs to identify the build (doc/12-risks.md, R-146).
    pub commit: String,
    /// Built from a tree with uncommitted code, so the commit alone does not describe it.
    pub dirty: bool,
    #[specta(type = specta_typescript::Number)]
    pub built_at: i64,
    pub repository: String,
    pub os: app_state::environment::OsInfo,
    /// The engine actually rendering this window — the one the user has to update when a
    /// CSS feature is missing, and the one Cogit cannot ship itself.
    pub renderer: String,
    pub git: String,
    pub rustc: String,
    pub tauri: String,
    /// The `gix` version the reads go through.
    pub git_library: String,
    pub log_path: String,
    pub log_dir: String,
    pub settings_path: String,
    pub displays: Vec<app_state::environment::DisplayInfo>,
}

/// The Rust half of the third-party licence list, written by `build.rs`.
const THIRD_PARTY_CRATES: &str = include_str!(concat!(env!("OUT_DIR"), "/third-party-crates.txt"));

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

/// A mutation waits for its turn in the repository's lane before it starts (P1.3).
///
/// Three clicks on push are three pushes, one after the other, in the order they landed —
/// not three `git push` processes racing for the same ref lock, and not two of them
/// silently dropped.
async fn mutating<T, F>(
    state: &std::sync::Arc<app_state::AppState>,
    repo: RepoId,
    kind: OperationKind,
    label: &'static str,
    work: F,
) -> Result<T, GitError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, GitError> + Send + 'static,
{
    mutating_titled(state, repo, kind, kind.title(), label, work).await
}

/// `mutating` with a footer line of its own, for work the kind's title says too little about.
async fn mutating_titled<T, F>(
    state: &std::sync::Arc<app_state::AppState>,
    repo: RepoId,
    kind: OperationKind,
    title: &str,
    label: &'static str,
    work: F,
) -> Result<T, GitError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, GitError> + Send + 'static,
{
    let permit = state.enqueue(repo, kind, title).await;
    let result = blocking(label, work).await;
    permit.finish(result.is_ok());
    result
}

/// Everything queued or running, for a panel that has just been opened again (P1.5).
#[tauri::command]
#[specta::specta]
pub async fn list_operations(
    state: tauri::State<'_, crate::AppContext>,
) -> Result<Vec<app_state::Operation>, GitError> {
    Ok(state.state.operations())
}

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

/// The webview's own clock: how long the user waited between an action and the screen
/// showing its result. Only the backend half is visible from Rust.
#[tauri::command]
#[specta::specta]
pub fn report_timing(label: String, ms: u32, detail: String) {
    crate::profile::ui(&label, u64::from(ms), &detail);
}

/// One line of the webview's log. `message` starts with the webview's own `+Nms`: a
/// batch lands at once, so the file's timestamp is when it arrived, not when it was said.
#[derive(Debug, Deserialize, specta::Type)]
pub struct WebviewLogLine {
    pub level: String,
    pub message: String,
    pub context: String,
}

/// The webview's own log lines, into the same file, in batches.
///
/// A JS error that only reaches the devtools console dies with the renderer — which is
/// exactly the moment it was worth keeping.
#[tauri::command]
#[specta::specta]
pub fn log_from_frontend(lines: Vec<WebviewLogLine>) {
    for WebviewLogLine {
        level,
        message,
        context,
    } in lines
    {
        match level.as_str() {
            "error" => tracing::error!(target: "cogit::webview", context, "{message}"),
            "warn" => tracing::warn!(target: "cogit::webview", context, "{message}"),
            "debug" => tracing::debug!(target: "cogit::webview", context, "{message}"),
            _ => tracing::info!(target: "cogit::webview", context, "{message}"),
        }
    }
}

/// What travels up the channel while a content search runs.
///
/// `Started` comes first and carries the id, so the panel can cancel a search long before
/// it has an answer — which is the point when the user is typing.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SearchChunk {
    Started {
        id: u32,
    },
    Matches {
        matches: Vec<git_engine::ContentMatch>,
    },
    Done {
        total: u32,
        cancelled: bool,
    },
}

/// Every file of a commit's tree: the Files panel's Unchanged switch on a commit.
#[tauri::command]
#[specta::specta]
pub async fn commit_tree_files(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
) -> Result<Vec<String>, GitError> {
    let app_state = state.state.clone();
    blocking("commit_tree_files", move || {
        app_state.tree_files(repo, &rev)
    })
    .await
}

/// Searches inside files, streaming matches as they are found.
///
/// Cancellable: the id arrives on the first chunk and `cancel_operation` stops it.
/// Binary files and anything over two megabytes are skipped without being opened.
#[tauri::command]
#[specta::specta]
pub async fn search_file_contents(
    state: tauri::State<'_, crate::AppContext>,
    cancellations: tauri::State<'_, std::sync::Arc<crate::operations::Cancellations>>,
    repo: RepoId,
    query: String,
    is_regex: bool,
    scope: git_engine::SearchScope,
    on_chunk: tauri::ipc::Channel<SearchChunk>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    let cancellations = std::sync::Arc::clone(&cancellations);

    let (id, cancel) = cancellations.start();
    let _ = on_chunk.send(SearchChunk::Started { id });

    let token = cancel.clone();
    let channel = on_chunk.clone();
    let counted = blocking("search_file_contents", move || {
        let request = git_engine::SearchRequest {
            query: &query,
            is_regex,
            scope,
        };

        let mut total = 0_u32;
        app_state.search_contents(repo, &request, &|| token.is_cancelled(), &mut |batch| {
            total = total.saturating_add(u32::try_from(batch.len()).unwrap_or(u32::MAX));
            let _ = channel.send(SearchChunk::Matches { matches: batch });
        })?;
        Ok(total)
    })
    .await;

    cancellations.finish(id);
    let total = counted?;

    let _ = on_chunk.send(SearchChunk::Done {
        total,
        cancelled: cancel.is_cancelled(),
    });
    Ok(())
}

/// Opens a submodule from its node in the tree: the panels follow it, the Repositories
/// panel does not gain an entry for it (doc/12-risks.md, R-109).
#[tauri::command(async)]
#[specta::specta]
pub async fn open_submodule(
    state: tauri::State<'_, crate::AppContext>,
    owner: RepoId,
    key: String,
) -> Result<RepoSummary, GitError> {
    let app_state = state.state.clone();
    blocking("open_submodule", move || {
        app_state.open_submodule(owner, &key)
    })
    .await
}

/// Repository ▸ Edit Git Config. `repo` is only read for the repository scope.
#[tauri::command]
#[specta::specta]
pub async fn read_git_config(
    state: tauri::State<'_, crate::AppContext>,
    repo: Option<RepoId>,
    scope: git_engine::ConfigScope,
) -> Result<git_engine::ConfigFile, GitError> {
    let app_state = state.state.clone();
    blocking("read_git_config", move || {
        app_state.config_file(repo, scope)
    })
    .await
}

/// Written only after `git config --file` has read the text back without complaint.
#[tauri::command]
#[specta::specta]
pub async fn write_git_config(
    state: tauri::State<'_, crate::AppContext>,
    repo: Option<RepoId>,
    scope: git_engine::ConfigScope,
    text: String,
    crlf: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    let save = {
        let app_state = app_state.clone();
        move || app_state.save_config_file(repo, scope, &text, crlf)
    };
    match repo {
        Some(id) => {
            mutating(
                &app_state,
                id,
                OperationKind::Other,
                "write_git_config",
                save,
            )
            .await
        }
        None => blocking("write_git_config", save).await,
    }
}

/// Run in the background after a repository opens; nothing in it changes the repository.
#[tauri::command]
#[specta::specta]
pub async fn repository_health(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<git_engine::HealthFinding>, GitError> {
    let app_state = state.state.clone();
    blocking("repository_health", move || app_state.health(repo)).await
}

/// The submodules directly under `parent`; empty `parent` means the top level.
#[tauri::command]
#[specta::specta]
pub async fn list_submodules(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    parent: String,
) -> Result<Vec<Submodule>, GitError> {
    let app_state = state.state.clone();
    let found = blocking("list_submodules", move || {
        app_state.submodules_under(repo, &parent)
    })
    .await?;
    tracing::debug!(repo = repo.0, submodules = found.len(), "submodules listed");
    Ok(found)
}

/// Stops a running read. `false` when it had already finished.
#[tauri::command]
#[specta::specta]
pub fn cancel_operation(
    cancellations: tauri::State<'_, std::sync::Arc<crate::operations::Cancellations>>,
    id: u32,
) -> bool {
    let stopped = cancellations.cancel(id);
    tracing::debug!(id, stopped, "cancel requested");
    stopped
}

/// Answered by the page to show it is still running while a close is pending.
///
/// Injected by `shutdown::watch`, not called from `frontend/`: the whole point is that
/// it works without the page knowing about it (problem 13).
#[tauri::command]
#[specta::specta]
pub fn closing_ping() {
    crate::shutdown::answered();
}

#[tauri::command]
#[specta::specta]
pub fn report_memory(sample: crate::profile::RendererMemory) {
    crate::profile::renderer(&sample);
}

/// The settings document as JSON text. Rust owns the file because the menu and the
/// logger read it before there is a window to ask.
#[tauri::command(async)]
#[specta::specta]
pub fn read_settings(state: tauri::State<'_, crate::AppContext>) -> String {
    app_state::settings::read_document(&state.config_dir).to_string()
}

#[tauri::command(async)]
#[specta::specta]
pub fn write_setting(
    state: tauri::State<'_, crate::AppContext>,
    key: String,
    value: String,
) -> Result<(), GitError> {
    // Parsed here rather than stored raw: the same file is read back by the logger and
    // the menu, and a malformed value would take both down with it.
    let parsed = serde_json::from_str(&value)
        .map_err(|err| GitError::Internal(format!("settings value is not JSON: {err}")))?;

    app_state::settings::write_key(&state.config_dir, &key, parsed)
        .map_err(|err| GitError::Internal(format!("cannot write settings: {err}")))
}

/// Async: it spawns `git --version` and reads the registry, neither of which belongs on
/// the main thread that a plain command runs on.
#[tauri::command]
#[specta::specta]
pub async fn app_info(
    window: tauri::Window,
    state: tauri::State<'_, crate::AppContext>,
) -> Result<AppInfo, GitError> {
    let log_path = state.log_path.clone();
    let settings_path = app_state::settings::path(&state.config_dir);
    let displays = displays(&window);

    blocking("app_info", move || {
        let webview = tauri::webview_version().ok();
        Ok(AppInfo {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            debug_build: cfg!(debug_assertions),
            commit: env!("COGIT_COMMIT").to_owned(),
            dirty: env!("COGIT_DIRTY") == "true",
            built_at: env!("COGIT_BUILT_AT").parse().unwrap_or_default(),
            repository: env!("CARGO_PKG_REPOSITORY").to_owned(),
            os: app_state::environment::os_info(),
            renderer: app_state::environment::renderer_label(
                std::env::consts::OS,
                webview.as_deref(),
            ),
            git: git_engine::git_version().unwrap_or_else(|_| "not found".to_owned()),
            rustc: env!("COGIT_RUSTC").to_owned(),
            tauri: tauri::VERSION.to_owned(),
            git_library: git_engine::gix_version().to_owned(),
            log_dir: log_path
                .parent()
                .map(|dir| dir.display().to_string())
                .unwrap_or_default(),
            log_path: log_path.display().to_string(),
            settings_path: settings_path.display().to_string(),
            displays,
        })
    })
    .await
}

/// The same monitor list `window_place` reads at start-up; read-only.
fn displays(window: &tauri::Window) -> Vec<app_state::environment::DisplayInfo> {
    let monitors = window.available_monitors().unwrap_or_else(|err| {
        tracing::warn!(error = ?err, context = "cannot list the monitors for About");
        Vec::new()
    });
    let primary = window.primary_monitor().ok().flatten();
    monitors
        .iter()
        .map(|monitor| app_state::environment::DisplayInfo {
            name: monitor.name().cloned(),
            width: monitor.size().width,
            height: monitor.size().height,
            scale: monitor.scale_factor(),
            primary: primary
                .as_ref()
                .is_some_and(|main| main.position() == monitor.position()),
        })
        .collect()
}

/// Help ▸ About ▸ Third-party licences. `frontend` is the list the Vite build shipped
/// beside the page; the dev server has none.
#[tauri::command]
#[specta::specta]
pub async fn open_third_party_licences(
    app: tauri::AppHandle,
    frontend: Option<String>,
) -> Result<(), GitError> {
    let text = app_state::licences::document(
        env!("CARGO_PKG_VERSION"),
        THIRD_PARTY_CRATES,
        frontend.as_deref(),
    );
    let path = blocking("open_third_party_licences", move || {
        app_state::licences::write(&std::env::temp_dir(), &text)
            .map_err(|err| GitError::Internal(format!("cannot write the licence list: {err}")))
    })
    .await?;
    tauri_plugin_opener::OpenerExt::opener(&app)
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(|err| GitError::Internal(format!("cannot open {}: {err}", path.display())))
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
        path = %summary.root,
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

/// Between two progress messages after the first: often enough for the scrollbar, rare
/// enough that fifty thousand commits are a handful of messages, not 250.
const PROGRESS_EVERY: std::time::Duration = std::time::Duration::from_millis(50);

/// The walk and its layout stay in Rust; the channel only says how far it got and the
/// rows go out by `graph_window` (R-193). Dropping the channel cancels the walk.
#[tauri::command]
#[specta::specta]
pub async fn load_commits(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    query: CommitQuery,
    on_progress: tauri::ipc::Channel<GraphProgress>,
) -> Result<Vec<git_engine::SkippedRef>, GitError> {
    let app_state = state.state.clone();
    let started = std::time::Instant::now();
    let generation = app_state.begin_graph();

    let (sent, skipped) = blocking("load_commits", move || {
        let mut sent = 0;
        let mut last: Option<std::time::Instant> = None;
        let result =
            app_state.build_graph(repo, &query, generation, DEFAULT_CHUNK_SIZE, |progress| {
                sent = progress.total;
                let due = progress.is_last || last.is_none_or(|at| at.elapsed() >= PROGRESS_EVERY);
                if !due {
                    return true;
                }
                last = Some(std::time::Instant::now());
                on_progress.send(progress).is_ok()
            });
        result.map(|skipped| (sent, skipped))
    })
    .await?;

    tracing::info!(
        repo = repo.0,
        commits = sent,
        skipped = skipped.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "commit graph streamed"
    );
    Ok(skipped)
}

/// Columns of the rows (`app_state::graph_wire`) in base64: one string for the
/// `postMessage` transport to carry, not a JSON array of numbers (R-192, R-194). Empty
/// once a newer graph replaced `generation`, as the answer would be for other rows.
#[tauri::command]
#[specta::specta]
pub async fn graph_window(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    generation: u32,
    start: u32,
    count: u32,
) -> Result<String, GitError> {
    let window = state.state.graph_window(repo, generation, start, count);
    let bytes = window
        .map(|w| app_state::graph_wire::encode(&w))
        .unwrap_or_default();
    Ok(diff_engine::base64(&bytes))
}

#[tauri::command]
#[specta::specta]
pub async fn graph_row_of(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    generation: u32,
    oid: String,
) -> Result<Option<u32>, GitError> {
    Ok(state.state.graph_row_of(repo, generation, &oid))
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
    ($name:ident, $method:ident, $kind:ident) => {
        #[tauri::command]
        #[specta::specta]
        pub async fn $name(
            state: tauri::State<'_, crate::AppContext>,
            repo: RepoId,
            paths: Vec<String>,
        ) -> Result<(), GitError> {
            let app_state = state.state.clone();
            mutating(
                &state.state,
                repo,
                OperationKind::$kind,
                stringify!($name),
                move || app_state.$method(repo, &paths),
            )
            .await
        }
    };
}

path_command!(stage_paths, stage_paths, Stage);
path_command!(unstage_paths, unstage_paths, Stage);
path_command!(discard_paths, discard_paths, Discard);
path_command!(add_to_gitignore, add_to_gitignore, Stage);
path_command!(delete_untracked, delete_untracked, Discard);

#[tauri::command]
#[specta::specta]
pub async fn commit(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    request: CommitRequest,
) -> Result<String, GitError> {
    let app_state = state.state.clone();
    let oid = mutating(
        &state.state,
        repo,
        OperationKind::Commit,
        "commit",
        move || app_state.commit(repo, &request),
    )
    .await?;

    tracing::info!(repo = repo.0, oid = %oid, "commit created");
    Ok(oid)
}

/// Off the main thread: the whole journal can be a hundred megabyte-sized entries.
#[tauri::command(async)]
#[specta::specta]
pub fn command_log(state: tauri::State<'_, crate::AppContext>) -> Vec<GitOutput> {
    state.state.command_log()
}

/// One entry in full. The notice that opened the window carried only its summary.
#[tauri::command(async)]
#[specta::specta]
pub fn command_outcome(state: tauri::State<'_, crate::AppContext>, id: u32) -> Option<GitOutput> {
    state.state.command_outcome(id)
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

/// Any entry from the journal, not only the newest (T5.7).
#[tauri::command]
#[specta::specta]
pub async fn undo_entry(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    id: u32,
) -> Result<SafetyEntry, GitError> {
    let app_state = state.state.clone();
    let entry = mutating(
        &state.state,
        repo,
        OperationKind::Undo,
        "undo_entry",
        move || app_state.undo_entry(repo, id),
    )
    .await?;

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
    let entry = mutating(
        &state.state,
        repo,
        OperationKind::Undo,
        "undo_last",
        move || app_state.undo_last(repo),
    )
    .await?;

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
    ($name:ident, $kind:ident) => {
        #[tauri::command]
        #[specta::specta]
        pub async fn $name(
            state: tauri::State<'_, crate::AppContext>,
            repo: RepoId,
        ) -> Result<(), GitError> {
            let app_state = state.state.clone();
            mutating(
                &state.state,
                repo,
                OperationKind::$kind,
                stringify!($name),
                move || app_state.$name(repo),
            )
            .await
        }
    };
}

repo_command!(abort_operation, Merge);
repo_command!(continue_operation, Merge);
repo_command!(skip_operation, Merge);

#[tauri::command]
#[specta::specta]
pub async fn merge(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    options: MergeOptions,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Merge,
        "merge",
        move || app_state.merge(repo, &options),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn rebase(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    options: RebaseOptions,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Rebase,
        "rebase",
        move || app_state.rebase(repo, &options),
    )
    .await
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
            mutating(
                &state.state,
                repo,
                OperationKind::Commit,
                stringify!($name),
                move || app_state.$name(repo, &commits),
            )
            .await
        }
    };
}

replay_command!(cherry_pick);
replay_command!(revert);

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

/// Each row not cached costs a `head`, a `branches` and a `status`, so it leaves the
/// async workers that carry IPC.
#[tauri::command]
#[specta::specta]
pub async fn repositories(
    state: tauri::State<'_, crate::AppContext>,
) -> Result<Vec<RepoOverview>, GitError> {
    let app_state = state.state.clone();
    blocking("repositories", move || Ok(app_state.overviews())).await
}

/// Answers with the repositories left open, which the caller would otherwise ask for next.
#[tauri::command]
#[specta::specta]
pub async fn close_repository(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<RepoOverview>, GitError> {
    let app_state = state.state.clone();
    // Off the main thread: stopping a watcher joins the thread that delivers its events,
    // and a join on the message loop is a frozen window (doc/12-risks.md, R-126).
    blocking("close_repository", move || {
        app_state.close_repository(repo);
        Ok(app_state.overviews())
    })
    .await
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
    mutating(
        &state.state,
        repo,
        OperationKind::Submodule,
        "update_submodule",
        move || app_state.update_submodule(repo, &path, init),
    )
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
    mutating(
        &state.state,
        repo,
        OperationKind::Stage,
        "stage_selection",
        move || app_state.stage_selection(repo, &request, reverse),
    )
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

/// The Blame window for `path` at `rev`, titled with the commit `rev` resolves to. The one
/// way blame opens, from every menu and button (#10).
#[tauri::command]
#[specta::specta]
pub async fn open_blame_window(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    rev: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    let oid = blocking("resolve_blame_revision", move || {
        app_state
            .commit_details(repo, &rev)
            .map(|details| details.oid)
    })
    .await?;

    blocking("open_blame_window", move || {
        crate::child_window::open_with_menu(
            &app,
            "blame",
            crate::blame_window::url(repo.0, &path, &oid),
            crate::blame_window::title(&path, &oid),
            crate::child_window::Shape {
                width: 1100.0,
                height: 800.0,
                min_width: 700.0,
                min_height: 450.0,
            },
            crate::blame_window::MENU,
        )
        .map_err(|err| GitError::Internal(format!("cannot open the blame window: {err}")))
    })
    .await
}

/// The commits that made line `line` of `path` at `rev` what it is, newest first.
#[tauri::command]
#[specta::specta]
pub async fn line_history(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    rev: String,
    line: u32,
) -> Result<Vec<git_engine::LineVersion>, GitError> {
    const LIMIT: usize = 200;
    let app_state = state.state.clone();
    blocking("line_history", move || {
        app_state.line_history(repo, &path, &rev, line, LIMIT)
    })
    .await
}

/// The versions of `path` up to `rev`: the commits that changed it, newest first.
#[tauri::command]
#[specta::specta]
pub async fn file_revisions(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    rev: String,
) -> Result<Vec<CommitRow>, GitError> {
    const LIMIT: usize = 500;
    let app_state = state.state.clone();
    blocking("file_revisions", move || {
        app_state.file_revisions(repo, &path, &rev, LIMIT)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn ref_dates(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<git_engine::RefDate>, GitError> {
    let app_state = state.state.clone();
    blocking("ref_dates", move || app_state.ref_dates(repo)).await
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

#[tauri::command]
#[specta::specta]
pub async fn rollback_to(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
    paths: Vec<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Undo,
        "rollback_to",
        move || app_state.rollback_to(repo, &rev, &paths),
    )
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
    mutating(
        &state.state,
        repo,
        OperationKind::Commit,
        "split_off",
        move || app_state.split_off(repo, &rev, &paths, &message, split_first),
    )
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
    mutating(
        &state.state,
        repo,
        OperationKind::Rebase,
        "interactive_rebase",
        move || app_state.interactive_rebase(repo, &base, &plan, paused),
    )
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

/// Off the main thread: building a window inside the WebView2 callback of a synchronous
/// command deadlocks every window (R-201). The parameters ride in the URL so the window
/// rebuilds itself after a webview reload (T2.5).
#[tauri::command]
#[specta::specta]
pub async fn open_compare_window(
    app: tauri::AppHandle,
    url: String,
    title: String,
) -> Result<(), GitError> {
    blocking("open_compare_window", move || {
        crate::child_window::open(
            &app,
            "compare",
            url,
            title,
            crate::child_window::Shape {
                width: 1000.0,
                height: 720.0,
                min_width: 600.0,
                min_height: 400.0,
            },
        )
        .map_err(|err| GitError::Internal(format!("cannot open the compare window: {err}")))
    })
    .await
}

/// Closes whichever window asked. In Rust rather than through `getCurrentWindow()`, which
/// keeps the call out of the webview (R-86); off the main thread for the reason in R-201.
#[tauri::command]
#[specta::specta]
pub async fn close_this_window(window: tauri::Window) -> Result<(), GitError> {
    window
        .close()
        .map_err(|err| GitError::Internal(format!("cannot close the window: {err}")))
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
    mutating(
        &state.state,
        repo,
        OperationKind::Stage,
        "stage_mode",
        move || app_state.stage_mode(repo, &path, executable),
    )
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
    mutating(
        &state.state,
        repo,
        OperationKind::Discard,
        "discard_selection",
        move || app_state.discard_selection(repo, &request),
    )
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

#[cfg(test)]
mod tests;
