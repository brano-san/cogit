//! `git bisect`: start, mark a commit, reset (M11). Each step checks out a commit, so it
//! waits in the repository's lane as a checkout does.

use super::mutating_titled;
use app_state::{OperationKind, RepoId};
use git_engine::{BisectMark, GitError};

#[tauri::command]
#[specta::specta]
pub async fn bisect_start(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    bad: String,
    good: Option<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating_titled(
        &state.state,
        repo,
        OperationKind::Checkout,
        "Starting bisect",
        "bisect_start",
        move || app_state.bisect_start(repo, &bad, good.as_deref()),
    )
    .await
}

/// `rev` is HEAD when absent.
#[tauri::command]
#[specta::specta]
pub async fn bisect_mark(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    mark: BisectMark,
    rev: Option<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    let title = match mark {
        BisectMark::Good => "Marking good",
        BisectMark::Bad => "Marking bad",
        BisectMark::Skip => "Skipping",
    };
    mutating_titled(
        &state.state,
        repo,
        OperationKind::Checkout,
        title,
        "bisect_mark",
        move || app_state.bisect_mark(repo, mark, rev.as_deref()),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn bisect_reset(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating_titled(
        &state.state,
        repo,
        OperationKind::Checkout,
        "Resetting bisect",
        "bisect_reset",
        move || app_state.bisect_reset(repo),
    )
    .await
}
