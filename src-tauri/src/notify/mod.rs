//! System notifications when an operation ends while the window is in the background.
//! The page decides whether to notify; this shows it and brings the window back on a click.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as platform;

#[cfg(not(windows))]
mod stub;
#[cfg(not(windows))]
use stub as platform;

pub use platform::register_app_id;

/// What a click on the notification hands back to the page.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct NotificationClicked {
    pub repo: Option<app_state::RepoId>,
    pub failed: bool,
}

pub fn show(
    app: &tauri::AppHandle,
    title: &str,
    body: &str,
    clicked: NotificationClicked,
) -> Result<(), String> {
    let handle = app.clone();
    platform::show(app, title, body, move || {
        use tauri::Manager as _;
        use tauri_specta::Event as _;
        if let Some(window) = handle.get_webview_window(crate::child_window::MAIN)
            && let Err(err) = window.unminimize().and_then(|()| window.set_focus())
        {
            tracing::warn!(error = %err, "cannot focus the main window from a notification");
        }
        if let Err(err) = clicked.emit(&handle) {
            tracing::error!(error = ?err, context = "notification click");
        }
    })
}
