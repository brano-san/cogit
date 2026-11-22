//! Native menus, title bars and dialogs follow either the chosen Cogit theme or the system.
//!
//! Under WSLg there is no desktop environment, so GTK always takes light Adwaita; the file
//! chooser and the menus would be white in a dark theme (doc/06-design-system.md, "Native
//! theme"). Fixed Cogit themes on Linux use GTK's built-in Adwaita (R-777).

use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use tauri::window::Color;
use tauri::{AppHandle, Manager as _, Runtime, Theme};

static THEME: AtomicU8 = AtomicU8::new(0);

/// The window background of the last `apply`, `0xRRGGBB`; `tauri.conf.json`'s until then.
static BACKGROUND: AtomicU32 = AtomicU32::new(0x0f_11_15);

/// The native preference is dark, light or the system default.
/// Every open window follows at once (`gtk-application-prefer-dark-theme` on Linux).
/// `background` is the theme's `bg.app`: GTK's menu bar is transparent over the window's
/// background, which stayed the dark one of `tauri.conf.json` in a light theme (R-777).
pub fn apply<R: Runtime>(app: &AppHandle<R>, dark: Option<bool>, background: &str) {
    let theme = dark.map_or(0, |dark| if dark { 2 } else { 1 });
    if THEME.swap(theme, Ordering::Relaxed) != theme {
        #[cfg(not(target_os = "linux"))]
        app.set_theme(current());
        #[cfg(target_os = "linux")]
        if let Err(err) = app.run_on_main_thread(move || apply_gtk_theme(dark)) {
            tracing::error!(error = ?err, context = "GTK color scheme");
        }
        #[cfg(target_os = "windows")]
        for window in app.webview_windows().values() {
            if let Err(err) = window.set_theme(current()) {
                tracing::error!(error = ?err, context = "window theme");
            }
        }
    }
    #[cfg(target_os = "windows")]
    if dark.is_none() {
        for window in app.webview_windows().values() {
            crate::webview2::follow_system_theme(window);
        }
    }
    let Some(rgb) = parse_rgb(background) else {
        tracing::warn!(
            background,
            "not a #rrggbb color; the window background stays"
        );
        return;
    };
    BACKGROUND.store(rgb, Ordering::Relaxed);
    #[cfg(target_os = "linux")]
    if let Err(err) = app.run_on_main_thread(move || paint_title_bars(rgb)) {
        tracing::error!(error = ?err, context = "title bar color for the theme");
    }
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

#[cfg(target_os = "linux")]
fn apply_gtk_theme(dark: Option<bool>) {
    use gtk::prelude::GtkSettingsExt;
    let Some(settings) = gtk::Settings::default() else {
        return;
    };
    if let Some(dark) = dark {
        settings.set_gtk_theme_name(Some("Adwaita"));
        settings.set_gtk_application_prefer_dark_theme(dark);
    } else {
        settings.reset_property("gtk-theme-name");
        settings.reset_property("gtk-application-prefer-dark-theme");
    }
}

#[cfg(target_os = "linux")]
pub fn initialize_gtk(taken: Option<&str>) {
    use gtk::prelude::GtkSettingsExt;
    let Some(settings) = gtk::Settings::default() else {
        tracing::warn!("no GTK settings: the native theme follows the desktop");
        return;
    };
    tracing::info!(
        gtk_theme_env = taken.unwrap_or("-"),
        theme_name = settings.gtk_theme_name().as_deref().unwrap_or("-"),
        prefer_dark = settings.is_gtk_application_prefer_dark_theme(),
        "native theme: GTK settings at startup"
    );
    // WSLg has no settings daemon: GTK's own default leaves the header bar only Close.
    if under_wsl() {
        settings.set_gtk_decoration_layout(Some("menu:minimize,maximize,close"));
    }
    paint_title_bars(BACKGROUND.load(Ordering::Relaxed));
}

/// `WSL_DISTRO_NAME` in a WSL shell; the interop entry when started another way.
#[cfg(target_os = "linux")]
fn under_wsl() -> bool {
    std::env::var_os("WSL_DISTRO_NAME").is_some()
        || std::path::Path::new("/proc/sys/fs/binfmt_misc/WSLInterop").exists()
}

/// On the main thread: GTK's header bar (Wayland, WSLg included, draws one on every
/// window) in the theme's background, as the menu bar under it; Adwaita's own is a gray
/// of its own in each variant. One style sheet, replaced on each theme change.
#[cfg(target_os = "linux")]
fn paint_title_bars(rgb: u32) {
    use gtk::prelude::CssProviderExt;
    thread_local! {
        static SHEET: std::cell::OnceCell<gtk::CssProvider> = const { std::cell::OnceCell::new() };
    }
    SHEET.with(|cell| {
        let sheet = cell.get_or_init(|| {
            let sheet = gtk::CssProvider::new();
            if let Some(screen) = gtk::gdk::Screen::default() {
                gtk::StyleContext::add_provider_for_screen(
                    &screen,
                    &sheet,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }
            sheet
        });
        let css = format!(
            ".titlebar {{ background: #{rgb:06x}; border-color: #{rgb:06x}; box-shadow: none; }}\n\
             menubar > menuitem:backdrop {{ color: @theme_fg_color; }}"
        );
        if let Err(err) = sheet.load_from_data(css.as_bytes()) {
            tracing::error!(error = ?err, context = "title bar style sheet");
        }
    });
}

pub fn set_window_theme<R: Runtime>(window: &tauri::WebviewWindow<R>) -> tauri::Result<()> {
    let theme = current();
    #[cfg(target_os = "linux")]
    if theme.is_none() {
        return Ok(());
    }
    window.set_theme(theme)
}

/// For a window being built: it opens in the theme the others already have.
#[must_use]
pub fn current() -> Option<Theme> {
    match THEME.load(Ordering::Relaxed) {
        2 => Some(Theme::Dark),
        1 => Some(Theme::Light),
        _ => None,
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
