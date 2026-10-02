//! What the backend tells the webview without being asked (doc/04-ipc-contract.md §6):
//! the event types, and the one task that carries `app_state`'s events across.

use app_state::AppState;
use std::sync::Arc;
use tauri_specta::Event as _;

/// Mirrors `app_state::AppEvent::RepoChanged`. It lives here because deriving
/// `tauri_specta::Event` would make `app_state` depend on tauri (INV-09).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct RepoChanged {
    pub repo: app_state::RepoId,
    pub kind: fs_watcher::ChangeKind,
}

/// A conflicted file was resolved in its own window; the main one refreshes on it.
/// The page emits it itself, like `RevealCommit`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct MergeResolved {
    pub repo: app_state::RepoId,
    pub path: String,
}

/// An external merge tool exited; the solver window and the main one both read it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct MergeToolFinished {
    pub repo: app_state::RepoId,
    pub path: String,
    pub outcome: app_state::MergeToolOutcome,
}

/// The Blame window asks the main one to select this commit and scroll the graph to it.
/// The page emits it itself: nothing on this side has to happen in between.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct RevealCommit {
    pub repo: app_state::RepoId,
    pub oid: String,
}

/// A native menu item was chosen. The payload is the palette command id, so the frontend
/// runs the same code path the palette would.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
pub struct MenuCommand(pub String);

/// Windows asked to end the session while operations run, and was told to wait.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct SessionEnding {
    pub reason: String,
}

/// Mirrors `app_state::AppEvent::AvatarReady`: one row can redraw without a refetch.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct AvatarReady {
    pub email: String,
}

/// Mirrors `app_state::AppEvent::CommandRecorded`. Every git command sends one; the
/// output itself stays in the journal until somebody asks to read it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct CommandRecorded(pub app_state::CommandNotice);

/// Mirrors `app_state::AppEvent::Operation`: the toolbar spinner and the queue indicator
/// read the same stream, one message per phase (P1.4).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct OperationChanged(pub app_state::Operation);

/// The watcher runs on its own thread, so events cross into the webview here.
pub(crate) fn forward_repo_changes(app: tauri::AppHandle, state: &Arc<AppState>) {
    let mut events = state.subscribe();
    #[cfg(windows)]
    let state = Arc::clone(state);
    tauri::async_runtime::spawn(async move {
        while let Some(event) = app_state::next_event(&mut events).await {
            match event {
                app_state::AppEvent::RepoChanged { repo, kind } => {
                    let _ = RepoChanged { repo, kind }.emit(&app);
                }
                app_state::AppEvent::CommandRecorded(notice) => {
                    let _ = CommandRecorded(notice).emit(&app);
                }
                app_state::AppEvent::AvatarReady { email } => {
                    let _ = AvatarReady { email }.emit(&app);
                }
                app_state::AppEvent::Operation(operation) => {
                    #[cfg(windows)]
                    if operation.phase == app_state::OperationPhase::Done
                        && state.session_end_blocker().is_none()
                    {
                        crate::session_end::release(&app);
                    }
                    let _ = OperationChanged(operation).emit(&app);
                }
                _ => {}
            }
        }
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ErrorKind {
    /// The command failed.
    Error,
    /// The command stopped halfway on conflicts: the user has work to do, nothing is broken.
    Warning,
}

/// One command that ended badly, as the Errors window lists it. The output itself stays in
/// the journal (`command_outcome(id)`): the window fetches it when the entry is on screen.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEntry {
    pub id: u32,
    pub kind: ErrorKind,
    pub title: String,
    pub operation: String,
    pub repo: String,
    pub command: String,
    pub summary: String,
    /// The same failure again collapses into this entry rather than growing the list.
    pub repeats: u32,
}

/// A page other than the main one hit a failed command: the main window owns the queue.
/// The page emits it itself.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct ErrorReported(pub ErrorEntry);

/// The whole queue, from the main window to the Errors window, on every change.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct ErrorQueue(pub Vec<ErrorEntry>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ErrorAction {
    /// The window is listening: send the queue.
    Ready,
    /// The entry `id` is on screen.
    Viewed,
    /// Remove the entry `id`.
    Dismiss,
    /// The window closed; everything it listed is dismissed.
    Closed,
    /// Take the main window to the conflicted files of the entry `id`.
    ShowConflicts,
}

/// From the Errors window to the main one.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct ErrorsAction {
    pub action: ErrorAction,
    pub id: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // The page reads `operation-changed` and `list_operations` as one type.
    #[test]
    fn an_operation_event_is_the_operation_itself() {
        let operation = app_state::Operation {
            id: 7,
            repo: Some(app_state::RepoId(1)),
            kind: app_state::OperationKind::Fetch,
            label: "Fetching".to_owned(),
            phase: app_state::OperationPhase::Done,
            success: Some(true),
        };
        assert_eq!(
            serde_json::to_value(OperationChanged(operation.clone())).unwrap(),
            serde_json::to_value(operation).unwrap()
        );
    }
}
