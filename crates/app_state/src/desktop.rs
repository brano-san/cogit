//! Open, reveal, shells and the bin. Each answer is data for a named platform, so every
//! platform's is tested on any machine; only `spawn` and `run` touch the system.

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOs,
    Linux,
}

impl Platform {
    #[must_use]
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Linux
        }
    }

    #[must_use]
    pub fn file_manager(self) -> &'static str {
        match self {
            Self::Windows => "Explorer",
            Self::MacOs => "Finder",
            Self::Linux => "File Manager",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub program: String,
    pub args: Vec<String>,
    /// Passed exactly as written. Explorer parses its own command line, and the usual
    /// quoting of `/select,C:\a b` into `"/select,C:\a b"` makes it open Documents.
    pub verbatim: bool,
    /// No console window of its own: `cmd /C start` only relays.
    pub hidden: bool,
}

impl Launch {
    fn plain(program: &str, args: &[&str]) -> Self {
        Self {
            program: program.to_owned(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            verbatim: false,
            hidden: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DesktopInfo {
    pub file_manager: String,
    /// PowerShell and Git Bash are Windows programs; elsewhere their items are left out.
    pub windows_shells: bool,
    pub git_shell: Option<String>,
    pub separator: String,
}

#[must_use]
pub fn info(platform: Platform) -> DesktopInfo {
    DesktopInfo {
        file_manager: platform.file_manager().to_owned(),
        windows_shells: platform == Platform::Windows,
        git_shell: find_git_bash().map(|path| path.to_string_lossy().into_owned()),
        separator: if platform == Platform::Windows {
            "\\"
        } else {
            "/"
        }
        .to_owned(),
    }
}

/// IPC paths use `/` everywhere; Windows tools want `\`, and no trailing one.
#[must_use]
pub fn native_path(platform: Platform, path: &str) -> String {
    let mut native = if platform == Platform::Windows {
        path.replace('/', "\\")
    } else {
        path.to_owned()
    };
    let separator = if platform == Platform::Windows {
        '\\'
    } else {
        '/'
    };
    while native.len() > 1 && native.ends_with(separator) && !native.ends_with(":\\") {
        native.pop();
    }
    native
}

#[must_use]
pub fn open_command(platform: Platform, path: &str) -> Launch {
    let native = native_path(platform, path);
    match platform {
        Platform::Windows => Launch {
            program: "explorer.exe".to_owned(),
            args: vec![format!("\"{native}\"")],
            verbatim: true,
            hidden: false,
        },
        Platform::MacOs => Launch::plain("open", &[&native]),
        Platform::Linux => Launch::plain("xdg-open", &[&native]),
    }
}

#[must_use]
pub fn reveal_command(platform: Platform, path: &str) -> Launch {
    let native = native_path(platform, path);
    match platform {
        Platform::Windows => Launch {
            program: "explorer.exe".to_owned(),
            args: vec![format!("/select,\"{native}\"")],
            verbatim: true,
            hidden: false,
        },
        Platform::MacOs => Launch::plain("open", &["-R", &native]),
        Platform::Linux => Launch::plain(
            "dbus-send",
            &[
                "--session",
                "--print-reply",
                "--type=method_call",
                "--dest=org.freedesktop.FileManager1",
                "/org/freedesktop/FileManager1",
                "org.freedesktop.FileManager1.ShowItems",
                &format!("array:string:{}", file_uri(&native)),
                "string:",
            ],
        ),
    }
}

/// `file://` plus the path, percent-encoded: a comma would split a `dbus-send` array.
#[must_use]
pub fn file_uri(path: &str) -> String {
    let mut uri = String::from("file://");
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) {
            uri.push(char::from(byte));
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri
}

/// The terminal's PowerShell; it inherits the folder, so no path is needed.
#[must_use]
pub fn power_shell_command(platform: Platform) -> Option<Launch> {
    use crate::terminal::{Terminal, launch_for};
    (platform == Platform::Windows).then(|| launch_for(platform, Terminal::PowerShell, "", None))
}

#[must_use]
pub fn git_shell_command(bash: &Path, platform: Platform, dir: &str) -> Launch {
    Launch {
        program: bash.to_string_lossy().into_owned(),
        args: vec![format!("--cd={}", native_path(platform, dir))],
        verbatim: false,
        hidden: false,
    }
}

/// Best guess first: the installer's record, the `git` on PATH, the program folders.
#[must_use]
pub fn git_bash_candidates(
    installs: &[PathBuf],
    git_on_path: Option<&Path>,
    program_dirs: &[PathBuf],
) -> Vec<PathBuf> {
    let from_git = git_on_path
        .into_iter()
        .flat_map(|git| git.ancestors().skip(1).take(3));
    let mut found: Vec<PathBuf> = Vec::new();
    let roots = installs
        .iter()
        .map(PathBuf::as_path)
        .chain(from_git)
        .map(Path::to_path_buf)
        .chain(program_dirs.iter().map(|dir| dir.join("Git")));
    for root in roots {
        let candidate = root.join("git-bash.exe");
        if !found.contains(&candidate) {
            found.push(candidate);
        }
    }
    found
}

#[must_use]
pub fn find_git_bash() -> Option<PathBuf> {
    if Platform::current() != Platform::Windows {
        return None;
    }
    let git = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join("git.exe"))
            .find(|git| git.is_file())
    });
    let program_dirs: Vec<PathBuf> = ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .chain(std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join("Programs")))
        .collect();
    let installs: Vec<PathBuf> = [true, false]
        .into_iter()
        .filter_map(git_engine::git_for_windows_install)
        .collect();
    git_bash_candidates(&installs, git.as_deref(), &program_dirs)
        .into_iter()
        .find(|path| path.is_file())
}

/// For `SHFileOperationW`: absolute, NUL after each, NUL at the end. A missing path is
/// refused here by name rather than left to a shell error code.
pub fn trash_list(paths: &[PathBuf]) -> std::io::Result<Vec<u16>> {
    let mut list = Vec::new();
    for path in paths {
        if std::fs::symlink_metadata(path).is_err() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("{} does not exist", path.display()),
            ));
        }
        let absolute = std::path::absolute(path)?;
        let native = native_path(Platform::Windows, &absolute.to_string_lossy());
        list.extend(native.encode_utf16());
        list.push(0);
    }
    list.push(0);
    Ok(list)
}

#[must_use]
pub fn trash_command(platform: Platform, paths: &[PathBuf]) -> Option<Launch> {
    let names: Vec<String> = paths
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    match platform {
        Platform::Windows => None,
        Platform::Linux => {
            let mut launch = Launch::plain("gio", &["trash", "--"]);
            launch.args.extend(names);
            Some(launch)
        }
        Platform::MacOs => {
            // Paths travel as argv of the script, never as its text: no byte of a name is code.
            let mut launch = Launch::plain(
                "osascript",
                &[
                    "-e",
                    "on run argv",
                    "-e",
                    "set fs to {}",
                    "-e",
                    "repeat with p in argv",
                    "-e",
                    "set end of fs to POSIX file (contents of p)",
                    "-e",
                    "end repeat",
                    "-e",
                    "tell application \"Finder\" to delete fs",
                    "-e",
                    "end run",
                ],
            );
            launch.args.extend(names);
            Some(launch)
        }
    }
}

/// Detached: the program outlives Cogit, so the caller does not wait. A thread collects it
/// when it ends, or it would stay a zombie until Cogit exits. Explorer in particular exits
/// with 1 on success, so its status would say nothing anyway.
pub fn spawn(launch: &Launch, cwd: Option<&Path>) -> std::io::Result<()> {
    let mut child = command_for(launch, cwd).spawn()?;
    std::thread::spawn(move || {
        if let Err(err) = child.wait() {
            tracing::warn!(error = ?err, "cannot collect a desktop program");
        }
    });
    Ok(())
}

/// Selects the item in the file manager. Without the `FileManager1` service on Linux
/// `dbus-send` fails, and the folder is opened instead.
pub fn reveal(platform: Platform, path: &str) -> std::io::Result<()> {
    let launch = reveal_command(platform, path);
    if platform != Platform::Linux {
        return spawn(&launch, None);
    }
    run(&launch).or_else(|err| {
        tracing::warn!(error = ?err, "no file manager service, opening the folder");
        spawn(&reveal_fallback(platform, path), None).map_err(|_| err)
    })
}

/// The parent folder, for a file manager that cannot select an item.
#[must_use]
pub fn reveal_fallback(platform: Platform, path: &str) -> Launch {
    let parent = Path::new(path).parent().and_then(Path::to_str);
    open_command(platform, parent.filter(|p| !p.is_empty()).unwrap_or(path))
}

pub fn run(launch: &Launch) -> std::io::Result<()> {
    let output = command_for(launch, None)
        .stderr(std::process::Stdio::piped())
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(std::io::Error::other(format!(
        "{} failed: {}",
        launch.program,
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

fn command_for(launch: &Launch, cwd: Option<&Path>) -> std::process::Command {
    use std::process::Stdio;
    let mut command = std::process::Command::new(&launch.program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        if launch.verbatim {
            for arg in &launch.args {
                command.raw_arg(arg);
            }
        } else {
            command.args(&launch.args);
        }
        if launch.hidden {
            command.creation_flags(CREATE_NO_WINDOW);
        }
    }
    #[cfg(not(windows))]
    command.args(&launch.args);
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    portable::restore_child_env(&mut command);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}
