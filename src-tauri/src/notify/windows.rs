//! Windows: a toast through WinRT, under the AppUserModelID both installers put on the Start
//! menu shortcut (the bundle `identifier`). Without that ID a toast shows under a wrong name or
//! not at all; the portable build has no shortcut, so the ID, its name and its icon are
//! registered under HKCU for every build (`register_identity`).

use parking_lot::Mutex;
use std::sync::OnceLock;
use windows::Data::Xml::Dom::XmlDocument;
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
use windows::core::{HSTRING, IInspectable};

static APP_ID: OnceLock<String> = OnceLock::new();
/// A toast dropped here stops answering clicks; the last few are kept alive.
static SHOWN: Mutex<Vec<ToastNotification>> = Mutex::new(Vec::new());
const KEPT: usize = 16;

/// Before the first window: the taskbar groups by it, and toasts are attributed to it.
#[allow(unsafe_code)]
pub fn register_app_id(id: &str) {
    if APP_ID.set(id.to_owned()).is_err() {
        return;
    }
    // SAFETY: a plain Win32 call with a valid, NUL-terminated wide string.
    if let Err(err) = unsafe { SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(id)) } {
        tracing::warn!(error = %err, "cannot set the AppUserModelID");
    }
}

/// The name and icon a toast shows. An unpackaged app has them from this registration, not
/// from the executable: without `IconUri` the toast drew a generic tile. The icon is written
/// next to the config, so the installed and the portable build both have a file to point at.
pub fn register_identity(config_dir: &std::path::Path) {
    let Some(id) = APP_ID.get() else {
        return;
    };
    let icon = config_dir.join("notification-icon.png");
    let written = std::fs::create_dir_all(config_dir)
        .and_then(|()| std::fs::write(&icon, ICON_PNG))
        .map_err(|err| err.to_string())
        .and_then(|()| {
            let key = windows_registry::CURRENT_USER
                .create(format!(r"Software\Classes\AppUserModelId\{id}"))
                .map_err(|err| err.to_string())?;
            key.set_string("DisplayName", "Cogit")
                .and_then(|()| key.set_string("IconUri", icon.to_string_lossy().as_ref()))
                .map_err(|err| err.to_string())
        });
    if let Err(err) = written {
        tracing::warn!(error = %err, "cannot register the name and icon for notifications");
    }
}

const ICON_PNG: &[u8] = include_bytes!("../../icons/128x128.png");

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn show(
    app: &tauri::AppHandle,
    title: &str,
    body: &str,
    on_click: impl Fn() + Send + Sync + 'static,
) -> Result<(), String> {
    let xml = format!(
        r#"<toast><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual></toast>"#,
        escape(title),
        escape(body)
    );
    let id = APP_ID.get().cloned().unwrap_or_else(|| "Cogit".to_owned());
    // WinRT wants an initialized apartment: the main thread has one.
    let (sent, answer) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let shown = (|| -> windows::core::Result<()> {
            let document = XmlDocument::new()?;
            document.LoadXml(&HSTRING::from(xml))?;
            let toast = ToastNotification::CreateToastNotification(&document)?;
            toast.Activated(&TypedEventHandler::<ToastNotification, IInspectable>::new(
                move |_, _| {
                    on_click();
                    Ok(())
                },
            ))?;
            ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(id))?
                .Show(&toast)?;
            let mut kept = SHOWN.lock();
            kept.push(toast);
            if kept.len() > KEPT {
                kept.remove(0);
            }
            Ok(())
        })();
        let _ = sent.send(shown.map_err(|err| err.to_string()));
    })
    .map_err(|err| err.to_string())?;
    answer
        .recv()
        .map_err(|err| err.to_string())
        .and_then(|shown| shown)
}
