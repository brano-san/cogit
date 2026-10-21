//! What still draws itself natively follows the Cogit theme, not the system's: the menu bar,
//! drop-down and context menus, the title bar on Windows, GTK dialogs on Linux.
//!
//! Under WSLg there is no desktop environment, so GTK always takes light Adwaita; the file
//! chooser and the menus would be white in a dark theme (doc/06-design-system.md, "Native
//! theme"). On Linux the theme is GTK's built-in Adwaita, whatever the desktop's (R-777).

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

/// `GTK_THEME` makes GTK ignore prefer-dark (GWSL exports `Adwaita:dark`): before GTK
/// starts the variable goes, and what it said is returned for the log.
#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
pub fn take_gtk_theme() -> Option<String> {
    let theme = std::env::var("GTK_THEME").ok()?;
    // SAFETY: called at the start of `run()`, while the process is single-threaded.
    unsafe { std::env::remove_var("GTK_THEME") };
    Some(theme)
}

/// On the main thread, once GTK runs. prefer-dark only switches a theme that has both
/// variants, and a desktop's may be dark only (`Yaru-dark`, a KDE color scheme): the menus
/// stayed black in a light Cogit theme. Adwaita is built into GTK and always has both.
#[cfg(target_os = "linux")]
pub fn use_adwaita(taken: Option<&str>) {
    use gtk::prelude::GtkSettingsExt;
    let Some(settings) = gtk::Settings::default() else {
        tracing::warn!("no GTK settings: the native theme follows the desktop");
        return;
    };
    tracing::info!(
        gtk_theme_env = taken.unwrap_or("-"),
        theme_name = settings.gtk_theme_name().as_deref().unwrap_or("-"),
        prefer_dark = settings.is_gtk_application_prefer_dark_theme(),
        "native theme: what GTK had, before Adwaita"
    );
    settings.set_gtk_theme_name(Some("Adwaita"));
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
