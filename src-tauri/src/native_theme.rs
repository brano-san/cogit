//! What still draws itself natively follows the Cogit theme, not the system's: the menu bar,
//! drop-down and context menus, the title bar on Windows, GTK dialogs on Linux.
//!
//! Under WSLg there is no desktop environment, so GTK always takes light Adwaita; the file
//! chooser and the menus would be white in a dark theme (doc/06-design-system.md, "Native
//! theme"). `gtk-theme-name` is left alone unless `GTK_THEME` pins it.

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

/// `GTK_THEME` makes GTK ignore prefer-dark, so the menus kept its variant whatever Cogit's
/// theme (GWSL exports `Adwaita:dark`). Before GTK starts the variable goes and its name is
/// returned for `keep_theme_name`; the variant then follows `apply`.
#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
pub fn take_gtk_theme() -> Option<String> {
    let theme = std::env::var("GTK_THEME").ok()?;
    // SAFETY: called at the start of `run()`, while the process is single-threaded.
    unsafe { std::env::remove_var("GTK_THEME") };
    theme_name(&theme)
}

/// On the main thread, once GTK runs: the name `take_gtk_theme` took, as the theme setting.
#[cfg(target_os = "linux")]
pub fn keep_theme_name(name: Option<&str>) {
    use gtk::prelude::GtkSettingsExt;
    if let (Some(name), Some(settings)) = (name, gtk::Settings::default()) {
        settings.set_gtk_theme_name(Some(name));
    }
}

/// `Adwaita:dark` → `Adwaita`; an empty name is no theme.
#[cfg(any(target_os = "linux", test))]
fn theme_name(gtk_theme: &str) -> Option<String> {
    let name = gtk_theme.split(':').next().unwrap_or_default();
    (!name.is_empty()).then(|| name.to_owned())
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

#[cfg(test)]
mod tests {
    use super::theme_name;

    #[test]
    fn gtk_theme_keeps_its_name_without_the_variant() {
        assert_eq!(theme_name("Adwaita:dark").as_deref(), Some("Adwaita"));
        assert_eq!(theme_name("Breeze").as_deref(), Some("Breeze"));
        assert_eq!(theme_name(":dark"), None);
        assert_eq!(theme_name(""), None);
    }
}
