//! The portable build (cargo feature `portable`): every file the app writes lives in
//! `Cogit-data` beside the binary (doc/13-distribution.md). In a normal build nothing here
//! runs: `activate` returns `None` and `portable::layout()` stays empty.
#![cfg_attr(not(feature = "portable"), allow(dead_code))]

use portable::Layout;

pub const ENABLED: bool = cfg!(feature = "portable");

/// First thing in `run()`: single-threaded, before GTK, Tauri and the logger read the
/// environment. A data folder that cannot be written stops the start (exit code 1,
/// message on stderr); the app never falls back to the system's folders.
pub fn activate() -> anyhow::Result<Option<&'static Layout>> {
    if !ENABLED {
        return Ok(None);
    }
    let folder = portable::locate()?;
    Ok(Some(portable::activate(Layout::in_folder(&folder))?))
}

/// The windows the config declares are created here, because Tauri places the webview
/// profile of a config window in the system's local data folder and accepts no absolute
/// path for it. Every window, children included, shares this one profile.
pub fn create_config_windows(app: &tauri::App, layout: &Layout) -> tauri::Result<()> {
    for config in &app.config().app.windows {
        tauri::WebviewWindowBuilder::from_config(app, config)?
            .data_directory(layout.local())
            .build()?;
    }
    Ok(())
}

pub fn hold_config_windows(context: &mut tauri::Context) {
    for window in &mut context.config_mut().app.windows {
        window.create = false;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[cfg(not(feature = "portable"))]
    #[test]
    fn a_normal_build_redirects_and_creates_nothing() {
        assert!(activate().unwrap().is_none());
        assert!(portable::layout().is_none());
    }

    #[cfg(feature = "portable")]
    #[test]
    fn the_portable_build_creates_the_config_windows_itself() {
        let mut context = tauri::generate_context!();
        assert!(!context.config().app.windows.is_empty());
        hold_config_windows(&mut context);
        assert!(context.config().app.windows.iter().all(|w| !w.create));
    }
}
