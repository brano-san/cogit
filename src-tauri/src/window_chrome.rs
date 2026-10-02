//! Who draws a window's titlebar and menus: the OS or the web layer.
//!
//! WSLg has no desktop environment, so GTK paints a light header bar and light menus over a
//! dark page. Linux therefore runs without native decorations and with the menus in the
//! page; Windows and macOS keep theirs (R-620, a macOS bar is the convention). One place
//! decides it, so a window, a menu and the page cannot disagree.

use std::path::Path;

use app_state::desktop::Platform;

/// `uiWebMenus` in the settings file: `auto` (absent) follows the platform.
const SETTING: &str = "uiWebMenus";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WindowChrome {
    /// Menu bar, drop-down and context menus are drawn by the page.
    pub web_menus: bool,
    /// The window has no native decorations; the page draws the titlebar.
    pub custom_titlebar: bool,
}

#[must_use]
pub fn resolve(platform: Platform, setting: Option<&str>) -> WindowChrome {
    let web_menus = match setting {
        Some("on") => true,
        Some("off") => false,
        _ => platform == Platform::Linux,
    };
    WindowChrome {
        web_menus,
        custom_titlebar: platform == Platform::Linux,
    }
}

/// Read before the first window exists, like the keymap: decorations cannot change later.
#[must_use]
pub fn stored(config_dir: &Path) -> WindowChrome {
    let document = app_state::settings::read_document(config_dir);
    let setting = document
        .get("settings")
        .and_then(|settings| settings.get(SETTING))
        .and_then(serde_json::Value::as_str);
    resolve(Platform::current(), setting)
}

/// What `setup` resolved; the platform default before that.
#[must_use]
pub fn of<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> WindowChrome {
    use tauri::Manager as _;
    app.try_state::<WindowChrome>()
        .map_or_else(|| resolve(Platform::current(), None), |held| *held)
}

/// The window's native menu bar stays built, so its accelerators keep working, and is
/// only hidden: the page draws the bar.
pub fn hide_native_menu<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
    if cfg!(target_os = "macos") {
        return;
    }
    if let Err(err) = window.hide_menu() {
        tracing::warn!(error = %err, "cannot hide the native menu bar");
    }
    #[cfg(target_os = "linux")]
    keep_gtk_menubar_hidden(window);
}

/// GTK shows a hidden widget again whenever the window is shown (`show_all`): the window is
/// created invisible and shown later, so `hide_menu` alone leaves a second menu bar above the
/// page's own. `no_show_all` makes the bar skip every later `show_all`; its accelerators keep
/// working, they belong to the window's accelerator group, not to the visible widget.
#[cfg(target_os = "linux")]
fn keep_gtk_menubar_hidden<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
    use gtk::glib::object::{Cast, ObjectExt};
    use gtk::prelude::{BinExt, ContainerExt, WidgetExt};

    fn walk(widget: &gtk::Widget) {
        if widget.is::<gtk::MenuBar>() {
            widget.set_no_show_all(true);
            widget.hide();
            return;
        }
        if let Some(container) = widget.downcast_ref::<gtk::Container>() {
            for child in container.children() {
                walk(&child);
            }
        }
    }

    let handle = window.clone();
    if let Err(err) = window.run_on_main_thread(move || {
        if let Some(child) = handle.gtk_window().ok().and_then(|window| window.child()) {
            walk(&child);
        }
    }) {
        tracing::warn!(error = %err, "cannot reach the GTK window to hide its menu bar");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_draws_its_own_chrome_by_default() {
        let chrome = resolve(Platform::Linux, None);
        assert!(chrome.web_menus && chrome.custom_titlebar);
    }

    #[test]
    fn windows_and_macos_stay_native_by_default() {
        for platform in [Platform::Windows, Platform::MacOs] {
            let chrome = resolve(platform, None);
            assert!(!chrome.web_menus && !chrome.custom_titlebar, "{platform:?}");
        }
    }

    #[test]
    fn the_setting_switches_menus_on_any_platform() {
        assert!(resolve(Platform::Windows, Some("on")).web_menus);
        assert!(!resolve(Platform::Linux, Some("off")).web_menus);
        assert!(!resolve(Platform::MacOs, Some("auto")).web_menus);
        assert!(resolve(Platform::Linux, Some("garbage")).web_menus);
    }

    #[test]
    fn the_titlebar_follows_decorations_not_the_menu_switch() {
        assert!(!resolve(Platform::Windows, Some("on")).custom_titlebar);
        assert!(resolve(Platform::Linux, Some("off")).custom_titlebar);
    }
}
