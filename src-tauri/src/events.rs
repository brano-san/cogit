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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct MergeResolved {
    pub repo: app_state::RepoId,
    pub path: String,
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
pub struct OperationChanged {
    pub id: u32,
    pub label: String,
    /// `None` until the phase is `done`.
    pub success: Option<bool>,
    /// `None` for work that belongs to no repository in particular.
    pub repo: Option<app_state::RepoId>,
    pub kind: app_state::OperationKind,
    pub phase: app_state::OperationPhase,
}

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
                    let _ = OperationChanged {
                        id: operation.id,
                        label: operation.label,
                        success: operation.success,
                        repo: operation.repo,
                        kind: operation.kind,
                        phase: operation.phase,
                    }
                    .emit(&app);
                }
                _ => {}
            }
        }
    });
}
