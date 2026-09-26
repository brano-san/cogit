//! Blame and the history of a file, of a line and of a fragment.

use super::blocking;
use app_state::RepoId;
use git_engine::{BlameLine, CommitRow, GitError};

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
