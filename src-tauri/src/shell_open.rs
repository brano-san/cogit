//! Open and reveal as the desktop does. Windows asks the shell, so a folder reaches the file
//! manager made the default (File Pilot, Directory Opus); elsewhere `app_state::desktop`
//! names the program.

use app_state::desktop::{self, Platform};

/// As a double click: a folder in the file manager, a file in the program paired with it.
pub fn open(path: &str) -> std::io::Result<()> {
    tracing::info!(path, "opening on the desktop");
    match desktop::open_command(Platform::current(), path) {
        Some(launch) => desktop::spawn(&launch, None),
        None => shell_execute(&desktop::native_path(Platform::Windows, path)),
    }
}

/// The item selected in its folder. On Windows `SHOpenFolderAndSelectItems` (opener plugin):
/// a default file manager that does not intercept it gets the folder at once, but the call
/// waits 20 s more for an Explorer window (File Pilot 0.8.5), so nobody waits for the thread.
pub fn reveal(path: &str) -> std::io::Result<()> {
    tracing::info!(path, "revealing in the file manager");
    let platform = Platform::current();
    if let Some(result) = desktop::reveal(platform, path) {
        return result;
    }
    let native = std::path::PathBuf::from(desktop::native_path(platform, path));
    std::fs::symlink_metadata(&native)?;
    std::thread::spawn(move || {
        if let Err(err) = tauri_plugin_opener::reveal_item_in_dir(&native) {
            tracing::warn!(error = %err, path = %native.display(), "cannot reveal in the file manager");
        }
    });
    Ok(())
}

/// The default verb, with no UI of the shell's own: a failure is reported like any other.
#[cfg(windows)]
#[allow(unsafe_code)]
fn shell_execute(path: &str) -> std::io::Result<()> {
    use windows::Win32::System::Com::{
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
    };
    use windows::Win32::UI::Shell::{
        SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, ShellExecuteExW,
    };
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows_core::{HSTRING, PCWSTR};

    let file = HSTRING::from(path);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: u32::try_from(std::mem::size_of::<SHELLEXECUTEINFOW>()).unwrap_or(u32::MAX),
        // A blocking-pool thread: the shell finishes before it returns, not on its own later.
        fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpFile: PCWSTR(file.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: COM is initialized for the call and released after it when this call took it;
    // `file` outlives the call and is the only pointer set.
    unsafe {
        let com = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        let result = ShellExecuteExW(&raw mut info);
        if com.is_ok() {
            CoUninitialize();
        }
        result.map_err(std::io::Error::other)
    }
}

#[cfg(not(windows))]
fn shell_execute(path: &str) -> std::io::Result<()> {
    Err(std::io::Error::other(format!(
        "no shell to open {path} with"
    )))
}

#[cfg(all(test, windows))]
mod tests {
    // Without `SEE_MASK_FLAG_NO_UI` the shell would put up its own message box and wait.
    #[test]
    fn a_missing_path_is_an_error_not_a_dialog_of_the_shell() {
        let missing = std::env::temp_dir().join(format!("cogit-open-{}-gone", std::process::id()));
        let path = missing.to_string_lossy().replace('\\', "/");
        assert!(super::open(&path).is_err());
        assert!(super::reveal(&path).is_err());
    }
}
