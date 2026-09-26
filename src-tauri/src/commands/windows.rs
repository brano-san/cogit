//! The menu bar and its shortcuts, context menus, and the windows no panel owns.

use super::blocking;
use git_engine::GitError;

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

/// Preferences ▸ Keyboard, while it records a shortcut: the menu lets every key through.
#[tauri::command]
#[specta::specta]
pub fn capture_keys(capture: tauri::State<'_, crate::key_capture::KeyCapture>, on: bool) {
    capture.set(on);
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
