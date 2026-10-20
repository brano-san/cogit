//! What still draws itself natively follows the Cogit theme, not the system's: the menu bar,
//! drop-down and context menus, the title bar on Windows, GTK dialogs on Linux.
//!
//! Under WSLg there is no desktop environment, so GTK always takes light Adwaita; the file
//! chooser and the menus would be white in a dark theme (doc/06-design-system.md, "Native
//! theme"). `gtk-theme-name` is left alone.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Runtime, Theme};

/// The theme of the last `apply`; dark until the page says otherwise, as `tauri.conf.json`.
static DARK: AtomicBool = AtomicBool::new(true);

/// Dark for the dark themes, light for the light ones. `dark` comes from the theme's family.
/// Every open window follows at once (`gtk-application-prefer-dark-theme` on Linux).
pub fn apply<R: Runtime>(app: &AppHandle<R>, dark: bool) {
    DARK.store(dark, Ordering::Relaxed);
    app.set_theme(Some(current()));
}

/// For a window being built: it opens in the theme the others already have.
#[must_use]
pub fn current() -> Theme {
    if DARK.load(Ordering::Relaxed) {
        Theme::Dark
    } else {
        Theme::Light
    }
}
