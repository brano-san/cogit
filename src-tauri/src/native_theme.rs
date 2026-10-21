//! What still draws itself natively follows the Cogit theme, not the system's: the menu bar,
//! drop-down and context menus, the title bar on Windows, GTK dialogs on Linux.
//!
//! Under WSLg there is no desktop environment, so GTK always takes light Adwaita; the file
//! chooser and the menus would be white in a dark theme (doc/06-design-system.md, "Native
//! theme"). On Linux the theme is GTK's built-in Adwaita, whatever the desktop's (R-777).

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use tauri::window::Color;
use tauri::{AppHandle, Manager as _, Runtime, Theme};

/// The theme of the last `apply`; dark until the page says otherwise, as `tauri.conf.json`.
static DARK: AtomicBool = AtomicBool::new(true);

/// The window background of the last `apply`, `0xRRGGBB`; `tauri.conf.json`'s until then.
static BACKGROUND: AtomicU32 = AtomicU32::new(0x0f_11_15);

/// Dark for the dark themes, light for the light ones. `dark` comes from the theme's family.
/// Every open window follows at once (`gtk-application-prefer-dark-theme` on Linux).
/// `background` is the theme's `bg.app`: GTK's menu bar is transparent over the window's
/// background, which stayed the dark one of `tauri.conf.json` in a light theme (R-777).
pub fn apply<R: Runtime>(app: &AppHandle<R>, dark: bool, background: &str) {
    DARK.store(dark, Ordering::Relaxed);
    app.set_theme(Some(current()));
    let Some(rgb) = parse_rgb(background) else {
        tracing::warn!(
            background,
            "not a #rrggbb color; the window background stays"
        );
        return;
    };
    BACKGROUND.store(rgb, Ordering::Relaxed);
    for window in app.webview_windows().values() {
        if let Err(err) = window.set_background_color(Some(self::background())) {
            tracing::error!(error = ?err, context = "window background for the theme");
        }
    }
}

/// For a window being built, as `current` is for its theme.
#[must_use]
pub fn background() -> Color {
    let [_, r, g, b] = BACKGROUND.load(Ordering::Relaxed).to_be_bytes();
    Color(r, g, b, 0xff)
}

/// `#rrggbb` as `0xRRGGBB`.
fn parse_rgb(hex: &str) -> Option<u32> {
    let digits = hex.strip_prefix('#')?;
    if digits.len() != 6 {
        return None;
    }
    u32::from_str_radix(digits, 16).ok()
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

#[cfg(test)]
mod tests {
    use super::parse_rgb;

    #[test]
    fn a_theme_background_is_read_as_rgb() {
        assert_eq!(parse_rgb("#f3f4f6"), Some(0xf3_f4_f6));
        assert_eq!(parse_rgb("#0F1115"), Some(0x0f_11_15));
        assert_eq!(parse_rgb("f3f4f6"), None);
        assert_eq!(parse_rgb("#fff"), None);
        assert_eq!(parse_rgb("#f3f4f6aa"), None);
    }
}
