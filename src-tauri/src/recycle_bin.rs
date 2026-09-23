//! Delete to the bin: `std::fs` only destroys; the shell knows where the bin is.

use std::path::PathBuf;

#[cfg(windows)]
#[allow(unsafe_code)]
pub fn move_to_trash(paths: &[PathBuf]) -> std::io::Result<()> {
    use windows::Win32::UI::Shell::{
        FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT,
        FOF_WANTNUKEWARNING, SHFILEOPSTRUCTW, SHFileOperationW,
    };
    use windows_core::PCWSTR;

    if paths.is_empty() {
        return Ok(());
    }
    let list = app_state::desktop::trash_list(paths)?;
    let flags =
        FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT | FOF_WANTNUKEWARNING;
    let mut operation = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: PCWSTR(list.as_ptr()),
        fFlags: u16::try_from(flags.0).unwrap_or(u16::MAX),
        ..Default::default()
    };
    // SAFETY: `list` is double-NUL-terminated and outlives the call; no other pointer is set.
    let code = unsafe { SHFileOperationW(&raw mut operation) };
    if code != 0 {
        return Err(std::io::Error::other(format!(
            "the shell could not move the files to the Recycle Bin (code {code:#x})"
        )));
    }
    if operation.fAnyOperationsAborted.as_bool() {
        return Err(std::io::Error::other(
            "moving to the Recycle Bin was cancelled",
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn move_to_trash(paths: &[PathBuf]) -> std::io::Result<()> {
    use app_state::desktop::{Platform, run, trash_command};
    if paths.is_empty() {
        return Ok(());
    }
    match trash_command(Platform::current(), paths) {
        Some(launch) => run(&launch),
        None => Err(std::io::Error::other("no bin on this platform")),
    }
}
