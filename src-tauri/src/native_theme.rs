//! What still draws itself natively follows the Cogit theme, not the system's.
//!
//! Under WSLg there is no desktop environment, so GTK always takes light Adwaita; the file
//! chooser of the dialog plugin and system notifications would be white in a dark theme
//! (doc/06-design-system.md, "Native theme"). `gtk-theme-name` is left alone.

use tauri::{AppHandle, Runtime};

/// Dark for the dark themes, light for the light ones. `dark` comes from the theme's family.
pub fn apply<R: Runtime>(app: &AppHandle<R>, dark: bool) {
    #[cfg(target_os = "linux")]
    {
        use gtk::prelude::GtkSettingsExt;
        // GTK is single-threaded: the settings object is only touched on the main thread.
        let queued = app.run_on_main_thread(move || {
            if let Some(settings) = gtk::Settings::default() {
                settings.set_gtk_application_prefer_dark_theme(dark);
            }
        });
        if let Err(err) = queued {
            tracing::warn!(%err, "could not queue the native theme change");
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = (app, dark);
}
