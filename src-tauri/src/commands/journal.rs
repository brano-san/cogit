//! What ran and what runs: the command journal, the safety journal with Undo, the queue.

use super::{blocking_or_default, mutating};
use app_state::{OperationKind, RepoId, SafetyEntry};
use git_engine::{GitError, GitOutput};

/// Everything queued or running, for a panel that has just been opened again (P1.5).
#[tauri::command]
#[specta::specta]
pub async fn list_operations(
    state: tauri::State<'_, crate::AppContext>,
) -> Result<Vec<app_state::Operation>, GitError> {
    Ok(state.state.operations())
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

/// In the blocking pool: the whole journal can be a hundred megabyte-sized entries.
#[tauri::command]
#[specta::specta]
pub async fn command_log(app: tauri::AppHandle) -> Vec<GitOutput> {
    let state = tauri::Manager::state::<crate::AppContext>(&app)
        .state
        .clone();
    blocking_or_default("command_log", move || state.command_log()).await
}

/// One entry in full. The notice that opened the window carried only its summary.
#[tauri::command]
#[specta::specta]
pub async fn command_outcome(app: tauri::AppHandle, id: u32) -> Option<GitOutput> {
    let state = tauri::Manager::state::<crate::AppContext>(&app)
        .state
        .clone();
    blocking_or_default("command_outcome", move || state.command_outcome(id)).await
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
