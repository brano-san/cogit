use app_state::{
    DEFAULT_CHUNK_SIZE, GraphProgress, OperationKind, RepoId, RepoOverview, RepoSummary,
    SafetyEntry,
};
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
    let permit = state.enqueue(repo, kind, kind.title()).await;
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

/// The webview's own log lines, into the same file.
///
/// A JS error that only reaches the devtools console dies with the renderer — which is
/// exactly the moment it was worth keeping.
#[tauri::command]
#[specta::specta]
pub fn log_from_frontend(level: String, message: String, context: String) {
    match level.as_str() {
        "error" => tracing::error!(target: "cogit::webview", context, "{message}"),
        "warn" => tracing::warn!(target: "cogit::webview", context, "{message}"),
        "debug" => tracing::debug!(target: "cogit::webview", context, "{message}"),
        _ => tracing::info!(target: "cogit::webview", context, "{message}"),
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

/// Every path in the repository: tracked plus untracked, never ignored.
///
/// Not the change list. The Files panel searches what changed; this is what lets it find
/// a file that nothing happened to, the way SmartGit does.
#[tauri::command]
#[specta::specta]
pub async fn list_all_repo_files(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<String>, GitError> {
    let app_state = state.state.clone();
    blocking("list_all_repo_files", move || app_state.all_files(repo)).await
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
    blocking("write_git_config", move || {
        app_state.save_config_file(repo, scope, &text, crlf)
    })
    .await
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

/// Everything `Help ▸ Copy Diagnostics` puts on the clipboard, as text.
#[tauri::command(async)]
#[specta::specta]
pub fn diagnostics(state: tauri::State<'_, crate::AppContext>) -> String {
    crate::diagnostics::report(
        &state.log_path,
        &state.config_dir,
        crate::webview2::browser_version().as_deref(),
    )
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

#[tauri::command]
#[specta::specta]
pub async fn checkout(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    target: CheckoutTarget,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Checkout,
        "checkout",
        move || app_state.checkout(repo, &target),
    )
    .await
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
    mutating(
        &state.state,
        repo,
        OperationKind::Branch,
        "create_branch",
        move || app_state.create_branch(repo, &name, start.as_deref(), switch_to),
    )
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
    mutating(
        &state.state,
        repo,
        OperationKind::Branch,
        "rename_branch",
        move || app_state.rename_branch(repo, &from, &to, force),
    )
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
    mutating(
        &state.state,
        repo,
        OperationKind::Branch,
        "set_upstream",
        move || app_state.set_upstream(repo, &branch, upstream.as_deref()),
    )
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
    mutating(
        &state.state,
        repo,
        OperationKind::Branch,
        "delete_remote_branch",
        move || app_state.delete_remote_branch(repo, &remote, &branch),
    )
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
    mutating(
        &state.state,
        repo,
        OperationKind::Branch,
        "delete_branch",
        move || app_state.delete_branch(repo, &name, force),
    )
    .await
}

#[tauri::command]
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
    let found = blocking("worktrees", move || app_state.worktrees(repo)).await?;
    tracing::info!(repo = repo.0, worktrees = found.len(), "worktrees listed");
    Ok(found)
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
    base: Option<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Worktree,
        "add_worktree",
        move || app_state.add_worktree(repo, &path, &branch, create, base.as_deref()),
    )
    .await
}

/// A worktree in the panels, not in the Repositories list (R-184).
#[tauri::command]
#[specta::specta]
pub async fn open_worktree(
    state: tauri::State<'_, crate::AppContext>,
    owner: RepoId,
    path: String,
) -> Result<RepoSummary, GitError> {
    let app_state = state.state.clone();
    blocking("open_worktree", move || {
        app_state.open_worktree(owner, &path)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn worktree_changes(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
) -> Result<Vec<git_engine::FileEntry>, GitError> {
    let app_state = state.state.clone();
    blocking("worktree_changes", move || {
        app_state.worktree_changes(repo, &path)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn prune_worktree(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Worktree,
        "prune_worktree",
        move || app_state.prune_worktree(repo, &path),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn repair_worktree(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Worktree,
        "repair_worktree",
        move || app_state.repair_worktree(repo, &path),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn lock_worktree(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    reason: Option<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Worktree,
        "lock_worktree",
        move || app_state.lock_worktree(repo, &path, reason.as_deref()),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn unlock_worktree(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Worktree,
        "unlock_worktree",
        move || app_state.unlock_worktree(repo, &path),
    )
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
    mutating(
        &state.state,
        repo,
        OperationKind::Worktree,
        "remove_worktree",
        move || app_state.remove_worktree(repo, &path, force),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn prune_worktrees(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Worktree,
        "prune_worktrees",
        move || app_state.prune_worktrees(repo),
    )
    .await
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
    mutating(
        &state.state,
        repo,
        OperationKind::Stash,
        "stash_selection",
        move || app_state.stash_selection(repo, &paths, &message),
    )
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
    mutating(
        &state.state,
        repo,
        OperationKind::Stash,
        "stash_push",
        move || app_state.stash_push(repo, &options),
    )
    .await
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
    mutating(
        &state.state,
        repo,
        OperationKind::Stash,
        "stash_apply",
        move || app_state.stash_apply(repo, index, pop),
    )
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
    mutating(
        &state.state,
        repo,
        OperationKind::Stash,
        "stash_drop",
        move || app_state.stash_drop(repo, index),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn create_tag(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    request: TagRequest,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Tag,
        "create_tag",
        move || app_state.create_tag(repo, &request),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_tag(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Tag,
        "delete_tag",
        move || app_state.delete_tag(repo, &name),
    )
    .await
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

#[tauri::command(async)]
#[specta::specta]
pub fn repositories(state: tauri::State<'_, crate::AppContext>) -> Vec<RepoOverview> {
    state.state.overviews()
}

#[tauri::command]
#[specta::specta]
pub async fn close_repository(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<bool, GitError> {
    let app_state = state.state.clone();
    // Off the main thread: stopping a watcher joins the thread that delivers its events,
    // and a join on the message loop is a frozen window (doc/12-risks.md, R-126).
    blocking("close_repository", move || {
        Ok(app_state.close_repository(repo))
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn submodules(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<Submodule>, GitError> {
    let app_state = state.state.clone();
    let found = blocking("submodules", move || app_state.submodules(repo)).await?;
    tracing::info!(repo = repo.0, submodules = found.len(), "submodules listed");
    Ok(found)
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
    mutating(
        &state.state,
        repo,
        OperationKind::Merge,
        "resolve_conflict",
        move || app_state.resolve_conflict(repo, &path, side),
    )
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
    mutating(
        &state.state,
        repo,
        OperationKind::Merge,
        "resolve_conflict_text",
        move || app_state.resolve_conflict_text(repo, &path, &text),
    )
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
}

/// Closes whichever window asked. Not `async`: window operations belong to the main
/// thread, and doing it here rather than through `getCurrentWindow()` keeps the call out
/// of the webview (doc/12-risks.md, R-86).
#[tauri::command]
#[specta::specta]
pub fn close_this_window(window: tauri::Window) -> Result<(), GitError> {
    window
        .close()
        .map_err(|err| GitError::Internal(format!("cannot close the window: {err}")))
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

/// The addresses on screen, for the download queue. Sent on every scroll, so it reads
/// nothing: rows that scrolled away leave the queue, the rest keep their place in it.
#[tauri::command]
#[specta::specta]
pub async fn avatar_window(
    state: tauri::State<'_, crate::AppContext>,
    emails: Vec<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("avatar_window", move || {
        app_state.avatar_window(&emails);
        Ok(())
    })
    .await
}

/// The pictures these authors already have. One file read and one base64 encode each,
/// so the caller asks only for what it does not hold (M14 T14.2).
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

#[tauri::command]
#[specta::specta]
pub async fn flow_status(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<git_engine::FlowStatus, GitError> {
    let app_state = state.state.clone();
    blocking("flow_status", move || app_state.flow_status(repo)).await
}

#[tauri::command]
#[specta::specta]
pub async fn flow_init(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    config: git_engine::FlowConfig,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Branch,
        "flow_init",
        move || app_state.flow_init(repo, &config),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn flow_start(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    kind: git_engine::FlowKind,
    name: String,
) -> Result<String, GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Branch,
        "flow_start",
        move || app_state.flow_start(repo, kind, &name),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn flow_finish(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    kind: git_engine::FlowKind,
    name: String,
    tag: Option<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Branch,
        "flow_finish",
        move || app_state.flow_finish(repo, kind, &name, tag.as_deref()),
    )
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
    crate::child_window::open(
        &app,
        "merge",
        url,
        title,
        crate::child_window::Shape {
            width: 1200.0,
            height: 760.0,
            min_width: 800.0,
            min_height: 500.0,
        },
    )
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

#[cfg(test)]
mod tests {
    /// A command declared `pub fn` is `ExecutionContext::Blocking`: it runs inline on the
    /// thread that delivered the IPC message, which on Windows is the thread pumping
    /// window messages. Anything slow there stops the window redrawing while it runs
    /// (problem 1). Only work that *must* stay on the main thread, or that is a handful of
    /// memory reads, belongs in this list.
    const MAIN_THREAD_ONLY: &[&str] = &[
        "default_keymap",
        "set_keymap",
        "set_menu_state",
        "report_timing",
        "log_from_frontend",
        "report_memory",
        "closing_ping",
        "cancel_operation",
        "terminal_choices",
        "command_log",
        "command_problems",
        "clear_command_log",
        "safety_log",
        "popup_context_menu",
        "open_compare_window",
        "open_merge_window",
        "close_this_window",
        "merge_resolved",
    ];

    /// Each command with a flag: does it leave the main thread? An `async fn` does, and so
    /// does a plain `fn` marked `#[tauri::command(async)]` — tauri hands that one to the
    /// thread pool without demanding it return a `Result`.
    fn declared() -> Vec<(String, bool)> {
        let mut found = Vec::new();
        let mut armed = false;
        let mut marked_async = false;
        for line in include_str!("mod.rs").lines() {
            let line = line.trim_start();
            if let Some(rest) = line.strip_prefix("#[tauri::command") {
                armed = true;
                marked_async = rest.starts_with("(async)");
                continue;
            }
            if !armed {
                continue;
            }
            if let Some(rest) = line.strip_prefix("pub async fn ") {
                found.push((name_of(rest), true));
                armed = false;
            } else if let Some(rest) = line.strip_prefix("pub fn ") {
                found.push((name_of(rest), marked_async));
                armed = false;
            }
        }
        found
    }

    fn name_of(rest: &str) -> String {
        rest.split(['(', '<']).next().unwrap_or_default().to_owned()
    }

    #[test]
    fn the_parser_sees_every_command() {
        let all = declared();
        assert!(
            all.len() > 60,
            "expected the whole command surface, parsed {}",
            all.len()
        );
        assert!(all.iter().any(|(name, _)| name == "repositories"));
    }

    #[test]
    fn nothing_heavy_runs_on_the_main_thread() {
        let stragglers: Vec<String> = declared()
            .into_iter()
            .filter(|(name, off_thread)| !off_thread && !MAIN_THREAD_ONLY.contains(&name.as_str()))
            .map(|(name, _)| name)
            .collect();

        assert!(
            stragglers.is_empty(),
            "these commands block the window's message loop; mark them \
             `#[tauri::command(async)]` or justify them in MAIN_THREAD_ONLY: {stragglers:?}"
        );
    }

    #[test]
    fn the_main_thread_list_has_no_leftovers() {
        let names: Vec<String> = declared().into_iter().map(|(name, _)| name).collect();
        let gone: Vec<&&str> = MAIN_THREAD_ONLY
            .iter()
            .filter(|allowed| !names.iter().any(|name| name == *allowed))
            .collect();

        assert!(gone.is_empty(), "no longer commands at all: {gone:?}");
    }
}
