//! The commit graph: the walk, windows of its rows and their paint, lookups over it (M4).

use super::blocking;
use app_state::{DEFAULT_CHUNK_SIZE, GraphProgress, RepoId};
use git_engine::{CommitQuery, CommitRow, Found, GitError};

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
    let app_state = state.state.clone();
    // Blocking: a window with texts nobody read yet reads up to `count` commits from disk.
    blocking("graph_window", move || {
        let window = app_state.graph_window(repo, generation, start, count);
        let bytes = window
            .map(|w| app_state::graph_wire::encode(&w))
            .unwrap_or_default();
        Ok(diff_engine::base64(&bytes))
    })
    .await
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

/// Colours and dimming for rows of graph `generation`, painted over the whole graph and
/// kept until the rows or the request change. `None` once a newer graph replaced it.
#[tauri::command]
#[specta::specta]
pub async fn graph_overlay(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    generation: u32,
    start: u32,
    count: u32,
    request: app_state::graph_overlay::GraphPaintRequest,
) -> Result<Option<app_state::graph_overlay::GraphOverlay>, GitError> {
    let app_state = state.state.clone();
    blocking("graph_overlay", move || {
        Ok(app_state.graph_overlay(repo, generation, start, count, &request))
    })
    .await
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
pub async fn find_object(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    query: String,
    limit: u32,
) -> Result<Vec<Found>, GitError> {
    let app_state = state.state.clone();
    blocking("find_object", move || app_state.find(repo, &query, limit)).await
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
