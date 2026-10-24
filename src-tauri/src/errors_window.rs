//! The Errors window: every failed git command, with its full output, in one window.
//! A command that stops on conflicts is listed there too, as a warning.

use tauri::Manager as _;

use crate::child_window::{self, Shape};

const KIND: &str = "errors";

/// Brings the window forward without taking focus from what the user is typing, or opens it.
/// Blocks while the window is built, so never call it on the main thread (R-201).
pub fn reveal_or_open<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    // A failure's notice clicked while the store opens it too: one looks while the other builds.
    static ONE_AT_A_TIME: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
    let _one = ONE_AT_A_TIME.lock();
    let open = app
        .webview_windows()
        .into_iter()
        .find(|(label, _)| label.starts_with(&format!("{KIND}-")));
    if let Some((_, window)) = open {
        window.unminimize()?;
        return window.show();
    }
    child_window::open(
        app,
        "errors",
        "errors.html".to_owned(),
        "Errors — Cogit".to_owned(),
        Shape {
            width: 860.0,
            height: 600.0,
            min_width: 520.0,
            min_height: 360.0,
        },
    )
}
