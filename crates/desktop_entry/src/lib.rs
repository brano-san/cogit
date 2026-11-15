//! The command line before any window: `cogit --install-desktop-entry` (what a portable
//! Linux binary needs so that the shell, WSLg included, finds a launcher entry and an icon),
//! `--add-to-path` for the Windows installer, and the paths the window is asked to open.

pub mod launch;
pub mod path_env;

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The name of the entry, the icon and the window class: all three must agree.
pub const APP_NAME: &str = "cogit";

const SIZES: [(&str, &[u8]); 7] = [
    (
        "16x16",
        include_bytes!("../../../src-tauri/icons/linux/hicolor/16x16/apps/cogit.png"),
    ),
    (
        "24x24",
        include_bytes!("../../../src-tauri/icons/linux/hicolor/24x24/apps/cogit.png"),
    ),
    (
        "32x32",
        include_bytes!("../../../src-tauri/icons/linux/hicolor/32x32/apps/cogit.png"),
    ),
    (
        "48x48",
        include_bytes!("../../../src-tauri/icons/linux/hicolor/48x48/apps/cogit.png"),
    ),
    (
        "64x64",
        include_bytes!("../../../src-tauri/icons/linux/hicolor/64x64/apps/cogit.png"),
    ),
    (
        "128x128",
        include_bytes!("../../../src-tauri/icons/linux/hicolor/128x128/apps/cogit.png"),
    ),
    (
        "256x256",
        include_bytes!("../../../src-tauri/icons/linux/hicolor/256x256/apps/cogit.png"),
    ),
];
const SVG: &[u8] = include_bytes!("../../../src-tauri/icons/linux/hicolor/scalable/apps/cogit.svg");

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Run,
    Install,
    Uninstall,
    Help,
    Version,
    AddToPath,
    RemoveFromPath,
}

pub fn parse_args<I: IntoIterator<Item = OsString>>(args: I) -> Action {
    for arg in args {
        match arg.to_str() {
            Some("--install-desktop-entry") => return Action::Install,
            Some("--uninstall-desktop-entry") => return Action::Uninstall,
            Some("--help" | "-h") => return Action::Help,
            Some("--version" | "-V") => return Action::Version,
            Some("--add-to-path") => return Action::AddToPath,
            Some("--remove-from-path") => return Action::RemoveFromPath,
            _ => {}
        }
    }
    Action::Run
}

/// One Exec argument per the Desktop Entry spec: reserved characters force double quotes,
/// `"` `` ` `` `$` `\` get a backslash, `%` is doubled; then the string layer doubles `\`.
pub fn exec_arg(arg: &str) -> String {
    const RESERVED: &str = " \t\n\"'\\><~|&;$*?#()`";
    let mut quoted = String::new();
    for c in arg.chars() {
        match c {
            '%' => quoted.push_str("%%"),
            '"' | '`' | '$' | '\\' => {
                quoted.push('\\');
                quoted.push(c);
            }
            _ => quoted.push(c),
        }
    }
    if arg.contains(|c| RESERVED.contains(c)) {
        quoted = format!("\"{quoted}\"");
    }
    quoted.replace('\\', "\\\\")
}

pub fn desktop_file(exe: &Path, comment: &str) -> String {
    let exec = exec_arg(&exe.to_string_lossy());
    format!(
        "[Desktop Entry]\nType=Application\nName=Cogit\nComment={comment}\nExec={exec} %F\n\
         Icon={APP_NAME}\nTerminal=false\nStartupWMClass={APP_NAME}\n\
         Categories=Development;RevisionControl;\nMimeType=inode/directory;\n\
         StartupNotify=true\n"
    )
}

/// `$XDG_DATA_HOME` when it is an absolute path (the spec ignores a relative one),
/// else `$HOME/.local/share`.
pub fn data_home(xdg: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    let xdg = xdg.map(PathBuf::from).filter(|p| p.is_absolute());
    xdg.or_else(|| {
        home.map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .map(|h| h.join(".local/share"))
    })
}

/// Where the entry goes: the user's data home, or `/usr/local/share` for root (`sudo`).
/// WSLg reads launcher entries only from the system folders, never from a home.
pub fn target_dir(root: bool, xdg: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    if root {
        return Some(PathBuf::from("/usr/local/share"));
    }
    data_home(xdg, home)
}

#[cfg(unix)]
fn running_as_root() -> bool {
    use std::os::unix::fs::MetadataExt as _;
    // `/proc/self` belongs to the effective user.
    std::fs::metadata("/proc/self").is_ok_and(|meta| meta.uid() == 0)
}

#[cfg(not(unix))]
fn running_as_root() -> bool {
    false
}

/// Everything the installer owns, relative to the data home.
fn payload(exe: &Path, comment: &str) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = vec![(
        PathBuf::from(format!("applications/{APP_NAME}.desktop")),
        desktop_file(exe, comment).into_bytes(),
    )];
    for (size, png) in SIZES {
        files.push((icon_path(size, "png"), png.to_vec()));
    }
    files.push((icon_path("scalable", "svg"), SVG.to_vec()));
    files
}

fn icon_path(dir: &str, ext: &str) -> PathBuf {
    PathBuf::from(format!("icons/hicolor/{dir}/apps/{APP_NAME}.{ext}"))
}

pub fn install(data_home: &Path, exe: &Path, comment: &str) -> io::Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for (rel, bytes) in payload(exe, comment) {
        let path = data_home.join(rel);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, bytes)?;
        written.push(path);
    }
    Ok(written)
}

/// Removes exactly the files `install` writes; the ones already gone are skipped.
pub fn uninstall(data_home: &Path) -> io::Result<Vec<PathBuf>> {
    let mut removed = Vec::new();
    for (rel, _) in payload(Path::new(""), "") {
        let path = data_home.join(rel);
        match std::fs::remove_file(&path) {
            Ok(()) => removed.push(path),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
    }
    Ok(removed)
}

/// Best effort: the tools are optional and the entry works without the caches.
fn refresh_caches(data_home: &Path) {
    let _ = Command::new("update-desktop-database")
        .arg(data_home.join("applications"))
        .status();
    let _ = Command::new("gtk-update-icon-cache")
        .args(["-q", "-t", "-f"])
        .arg(data_home.join("icons/hicolor"))
        .status();
}

const HELP: &str = "Usage: cogit [OPTIONS] [PATH]...\n       \
    cogit log [PATH]\n       \
    cogit blame FILE\n\n\
    Commands:\n  \
    PATH...      Open the repository at or above each path; a running Cogit takes it over\n  \
    log [PATH]   Open the repository and show the history of PATH (default: this folder)\n  \
    blame FILE   Open the repository and show Blame for FILE\n  \
    -- PATH...   Open folders named like a command or an option\n\n\
    Options:\n  \
    --install-desktop-entry    Add the launcher entry and icons to the user's data directory, or /usr/local/share as root (Linux)\n  \
    --uninstall-desktop-entry  Remove them again (Linux)\n  \
    --add-to-path              Add this binary's folder to the user's PATH (Windows)\n  \
    --remove-from-path         Remove it from the user's PATH again (Windows)\n  \
    -V, --version              Print the version and exit\n  \
    -h, --help                 Print this help and exit\n";

/// `Some(exit code)` when the command line asks for something other than the window.
pub fn handle<I: IntoIterator<Item = OsString>>(
    args: I,
    version: &str,
    comment: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Option<i32> {
    let args: Vec<OsString> = args.into_iter().collect();
    let code = match parse_args(args.clone()) {
        Action::Run => match launch::parse(args) {
            Ok(_) => return None,
            Err(message) => writeln!(err, "{message}\n\n{HELP}").map_or(1, |()| 2),
        },
        Action::Version => writeln!(out, "cogit {version}").map_or(1, |()| 0),
        Action::Help => write!(out, "{HELP}").map_or(1, |()| 0),
        action @ (Action::AddToPath | Action::RemoveFromPath) => manage_path(&action, out, err),
        action => manage(&action, comment, out, err),
    };
    Some(code)
}

/// An AppImage runs from a temporary mount: its own file, `$APPIMAGE`, is the launcher.
fn launcher_path() -> io::Result<PathBuf> {
    match std::env::var_os("APPIMAGE").map(PathBuf::from) {
        Some(image) if image.is_absolute() => Ok(image),
        _ => std::env::current_exe().and_then(|exe| exe.canonicalize()),
    }
}

#[cfg(windows)]
fn manage_path(action: &Action, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let add = *action == Action::AddToPath;
    let changed = std::env::current_exe().and_then(|exe| {
        let dir = exe
            .parent()
            .ok_or_else(|| io::Error::other("the binary has no folder"))?
            .to_string_lossy()
            .into_owned();
        path_env::change_user_path(&dir, add).map(|changed| (dir, changed))
    });
    match changed {
        Ok((dir, changed)) => {
            let verb = match (add, changed) {
                (true, true) => "Added to",
                (false, true) => "Removed from",
                (true, false) => "Already on",
                (false, false) => "Not on",
            };
            let _ = writeln!(out, "{verb} the user's PATH: {dir}");
            0
        }
        Err(e) => {
            let _ = writeln!(err, "cogit: {e}");
            1
        }
    }
}

#[cfg(not(windows))]
fn manage_path(_: &Action, _: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let _ = writeln!(err, "cogit: outside Windows the package puts cogit on PATH");
    1
}

fn manage(action: &Action, comment: &str, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    if !cfg!(target_os = "linux") {
        let _ = writeln!(err, "cogit: desktop entries exist on Linux only");
        return 1;
    }
    let Some(home) = target_dir(
        running_as_root(),
        std::env::var_os("XDG_DATA_HOME"),
        std::env::var_os("HOME"),
    ) else {
        let _ = writeln!(
            err,
            "cogit: neither XDG_DATA_HOME nor HOME is an absolute path"
        );
        return 1;
    };
    let result = if *action == Action::Install {
        launcher_path()
            .and_then(|exe| install(&home, &exe, comment))
            .map(|files| ("Installed", files))
    } else {
        uninstall(&home).map(|files| ("Removed", files))
    };
    match result {
        Ok((verb, files)) => {
            for file in &files {
                let _ = writeln!(out, "{verb} {}", file.display());
            }
            if files.is_empty() {
                let _ = writeln!(out, "Nothing to remove under {}", home.display());
            }
            refresh_caches(&home);
            0
        }
        Err(e) => {
            let _ = writeln!(err, "cogit: {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests;
