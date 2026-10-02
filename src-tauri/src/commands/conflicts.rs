//! Conflicts and the 3-way merge window (M7).

use super::{blocking, mutating};
use app_state::{OperationKind, RepoId};
use git_engine::{ConflictSide, GitError};

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

/// The Conflict Solver for one file: a window of its own, or the one already open for it.
/// `external_tool` has it start the merge tool as soon as it is up.
#[tauri::command]
#[specta::specta]
pub async fn open_solver_window(
    app: tauri::AppHandle,
    repo: RepoId,
    path: String,
    external_tool: bool,
) -> Result<(), GitError> {
    blocking("open_solver_window", move || {
        crate::solver_window::reveal_or_open(&app, repo.0, &path, external_tool)
    })
    .await
}

/// The three sides, merged by stretch, with the names of "ours" and "theirs".
#[tauri::command]
#[specta::specta]
pub async fn solver_data(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
) -> Result<app_state::SolverData, GitError> {
    let app_state = state.state.clone();
    blocking("solver_data", move || app_state.solver_data(repo, &path)).await
}

/// `git add` of a working file settled outside the solver (an external tool).
#[tauri::command]
#[specta::specta]
pub async fn mark_conflict_resolved(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Merge,
        "mark_conflict_resolved",
        move || app_state.mark_conflict_resolved(repo, &path),
    )
    .await
}

/// Starts the external tool and returns at once; `MergeToolFinished` says how it ended.
/// No program means git's `merge.tool`.
#[tauri::command]
#[specta::specta]
pub async fn launch_merge_tool(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    program: String,
    args: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("launch_merge_tool", move || {
        use tauri_specta::Event as _;
        let announced = path.clone();
        app_state.start_merge_tool(
            repo,
            &path,
            &program,
            &args,
            &std::env::temp_dir(),
            move |outcome| {
                let finished = crate::MergeToolFinished {
                    repo,
                    path: announced,
                    outcome,
                };
                if let Err(err) = finished.emit(&app) {
                    tracing::error!(error = ?err, context = "announcing the merge tool's end");
                }
            },
        )
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_merge_tool(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
) -> Result<bool, GitError> {
    let app_state = state.state.clone();
    blocking("cancel_merge_tool", move || {
        Ok(app_state.cancel_merge_tool(repo, &path))
    })
    .await
}

/// The files of `repo` an external tool is open on: a solver reloaded meanwhile stays locked.
#[tauri::command]
#[specta::specta]
pub async fn merge_tools_running(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<String>, GitError> {
    let app_state = state.state.clone();
    blocking("merge_tools_running", move || {
        Ok(app_state.merge_tools_running(repo))
    })
    .await
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
