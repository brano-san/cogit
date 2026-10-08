//! Windows: a toast through WinRT, under the AppUserModelID both installers put on the Start
//! menu shortcut (the bundle `identifier`). The shell takes a toast's name and icon from that
//! shortcut: without one a toast is dropped, or shows a blank or inverted tile the shell then
//! keeps. The portable and the dev build have no installer, so they make the shortcut
//! themselves (`register_identity`).

use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use windows::Data::Xml::Dom::XmlDocument;
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, IPersistFile, STGM_READ,
};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::Win32::UI::Shell::{
    FOLDERID_CommonPrograms, FOLDERID_Programs, IShellLinkW, KF_FLAG_DEFAULT, SHGetKnownFolderPath,
    SetCurrentProcessExplicitAppUserModelID, ShellLink,
};
use windows::core::{GUID, HSTRING, IInspectable, Interface};

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

/// A Start menu shortcut to this executable under the app's AppUserModelID, unless an
/// installer's is there. A shortcut of ours left pointing at a moved or deleted portable
/// copy is pointed here again.
pub fn register_identity(config_dir: &Path) {
    let Some(id) = APP_ID.get() else {
        return;
    };
    forget_registry_identity(id, config_dir);
    if let Err(err) = ensure_shortcut(id) {
        tracing::warn!(error = %err, "cannot make the Start menu shortcut notifications need");
    }
}

/// Earlier builds named the app for toasts under HKCU instead; the shell rendered that icon
/// inverted and kept it, so the entry and its icon file go.
fn forget_registry_identity(id: &str, config_dir: &Path) {
    let key = format!(r"Software\Classes\AppUserModelId\{id}");
    if windows_registry::CURRENT_USER.open(&key).is_ok()
        && let Err(err) = windows_registry::CURRENT_USER.remove_tree(&key)
    {
        tracing::warn!(error = %err, "cannot remove the old notification registration");
    }
    let _ = std::fs::remove_file(config_dir.join("notification-icon.png"));
}

#[allow(unsafe_code)]
fn ensure_shortcut(id: &str) -> windows::core::Result<()> {
    let exe = std::env::current_exe()
        .map_err(|err| windows::core::Error::new(windows::core::HRESULT(-1), err.to_string()))?;
    let ours = known_folder(&FOLDERID_Programs)?.join(format!("{id}.lnk"));
    let installed = [
        known_folder(&FOLDERID_Programs)?
            .join(id)
            .join(format!("{id}.lnk")),
        known_folder(&FOLDERID_CommonPrograms)?.join(format!("{id}.lnk")),
        known_folder(&FOLDERID_CommonPrograms)?
            .join(id)
            .join(format!("{id}.lnk")),
    ];
    if installed.iter().any(|path| path.exists()) {
        return Ok(());
    }
    // The main thread already has a COM apartment; this only makes sure.
    // SAFETY: no reserved pointer; a second call on the thread is harmless.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if ours.exists() && shortcut_target(&ours)?.is_some_and(|target| target.exists()) {
        return Ok(());
    }
    write_shortcut(&ours, &exe, id)
}

#[allow(unsafe_code)]
fn known_folder(folder: &GUID) -> windows::core::Result<PathBuf> {
    // SAFETY: the returned string is copied, then freed with the allocator that made it.
    unsafe {
        let raw = SHGetKnownFolderPath(folder, KF_FLAG_DEFAULT, None)?;
        let path = raw.to_string();
        CoTaskMemFree(Some(raw.0 as *const _));
        Ok(PathBuf::from(
            path.map_err(|_| windows::core::Error::empty())?,
        ))
    }
}

#[allow(unsafe_code)]
fn shortcut_target(lnk: &Path) -> windows::core::Result<Option<PathBuf>> {
    // SAFETY: COM calls on interfaces this function owns; the buffer outlives the call.
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.cast::<IPersistFile>()?
            .Load(&HSTRING::from(lnk.as_os_str()), STGM_READ)?;
        let mut buf = [0u16; 1024];
        link.GetPath(&mut buf, std::ptr::null_mut(), 0)?;
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Ok((len > 0).then(|| PathBuf::from(String::from_utf16_lossy(&buf[..len]))))
    }
}

#[allow(unsafe_code)]
fn write_shortcut(lnk: &Path, exe: &Path, id: &str) -> windows::core::Result<()> {
    // SAFETY: COM calls on interfaces this function owns, with valid wide strings.
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.SetPath(&HSTRING::from(exe.as_os_str()))?;
        if let Some(dir) = exe.parent() {
            link.SetWorkingDirectory(&HSTRING::from(dir.as_os_str()))?;
        }
        let store: IPropertyStore = link.cast()?;
        store.SetValue(&PKEY_AppUserModel_ID, &PROPVARIANT::from(id))?;
        store.Commit()?;
        link.cast::<IPersistFile>()?
            .Save(&HSTRING::from(lnk.as_os_str()), true)?;
    }
    tracing::info!(shortcut = %lnk.display(), "made the Start menu shortcut notifications need");
    Ok(())
}

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
