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

/// The page's taskbar signals, merged by `app_state::taskbar::resolve`. Main window only:
/// a child window's call is ignored rather than painting the shared button.
#[tauri::command]
#[specta::specta]
pub async fn set_taskbar_state(window: tauri::Window, signals: app_state::taskbar::TaskbarSignals) {
    if window.label() != crate::child_window::MAIN {
        return;
    }
    let shown = app_state::taskbar::resolve(&signals);
    if let Err(err) = crate::taskbar::apply(&window, &shown) {
        tracing::error!(error = ?err, context = "setting the taskbar state");
    }
}

/// A failed command opens the Errors window, or shows the one that is open.
#[tauri::command]
#[specta::specta]
pub async fn open_errors_window(app: tauri::AppHandle) -> Result<(), GitError> {
    blocking("open_errors_window", move || {
        crate::errors_window::reveal_or_open(&app)
            .map_err(|err| GitError::Internal(format!("cannot open the Errors window: {err}")))
    })
    .await
}

/// `Show conflicts` in the Errors window: the main window comes forward.
#[tauri::command]
#[specta::specta]
pub async fn focus_main_window(app: tauri::AppHandle) -> Result<(), GitError> {
    use tauri::Manager as _;
    let window = app
        .get_webview_window(crate::child_window::MAIN)
        .ok_or_else(|| GitError::Internal("the main window is gone".to_owned()))?;
    window
        .unminimize()
        .and_then(|()| window.set_focus())
        .map_err(|err| GitError::Internal(format!("cannot focus the main window: {err}")))
}

/// Who draws the titlebar and the menus of this build: the page asks once at start.
#[tauri::command]
#[specta::specta]
pub fn window_chrome(
    chrome: tauri::State<'_, crate::window_chrome::WindowChrome>,
) -> crate::window_chrome::WindowChrome {
    *chrome
}

/// The bar the calling window draws itself, from the tables the native bar is built from.
#[tauri::command]
#[specta::specta]
pub fn menu_model(
    window: tauri::Window,
    keymap: tauri::State<'_, crate::menu::Keymap>,
    items: tauri::State<'_, crate::menu::MenuItems<tauri::Wry>>,
) -> Vec<crate::menu::MenuNode> {
    crate::menu::model_for(window.label(), &keymap, &items)
}

/// A click in the page's own menu: the path a native menu event takes.
#[tauri::command]
#[specta::specta]
pub async fn menu_command(app: tauri::AppHandle, id: String) {
    crate::dispatch_menu_command(&app, &id);
}
