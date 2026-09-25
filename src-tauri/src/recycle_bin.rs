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

#[cfg(all(test, windows))]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::move_to_trash;

    fn scratch(name: &str) -> std::path::PathBuf {
        let unique = format!("cogit-bin-{}-{name}", std::process::id());
        std::env::temp_dir().join(unique)
    }

    #[test]
    fn a_file_moved_to_the_bin_is_gone_from_its_folder() {
        let file = scratch("gone.txt");
        std::fs::write(&file, "to the Recycle Bin\n").unwrap();

        move_to_trash(std::slice::from_ref(&file)).unwrap();

        assert!(!file.exists());
    }

    #[test]
    fn a_refusal_of_the_shell_comes_back_as_an_error() {
        let missing = scratch("never-there.txt");

        assert!(move_to_trash(&[missing]).is_err());
    }
}
