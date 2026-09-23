//! What the desktop does with a folder or a file: open it, reveal it in its parent, open a
//! shell in it, move it to the bin. Each answer is data for a named platform, so the macOS
//! and Linux ones are tested on Windows too; only `spawn` touches the system.

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOs,
    /// Every other Unix desktop: the freedesktop tools are what it has in common.
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

    /// What the menus call it: "Reveal in Explorer", "Reveal in Finder".
    #[must_use]
    pub fn file_manager(self) -> &'static str {
        match self {
            Self::Windows => "Explorer",
            Self::MacOs => "Finder",
            Self::Linux => "File Manager",
        }
    }
}

/// A program to start and forget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub program: String,
    pub args: Vec<String>,
    /// Passed exactly as written. Explorer parses its own command line, and the usual
    /// quoting of `/select,C:\a b` into `"/select,C:\a b"` makes it open Documents.
    pub verbatim: bool,
    /// Started without a console window of its own: `cmd /C start` only relays.
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

/// The platform's shell actions, for the menus that offer them.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DesktopInfo {
    pub file_manager: String,
    /// PowerShell and Git Bash are Windows programs; elsewhere their items are left out.
    pub windows_shells: bool,
    /// Git Bash from Git for Windows, when one was found.
    pub git_shell: Option<String>,
    /// What "Copy Path" joins with.
    pub separator: String,
}

#[must_use]
pub fn info() -> DesktopInfo {
    let platform = Platform::current();
    DesktopInfo {
        file_manager: platform.file_manager().to_owned(),
        windows_shells: power_shell_command(platform).is_some(),
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

/// A folder opens itself; a file opens in the application the desktop pairs it with.
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

/// The parent folder, with the item selected in it.
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

/// Windows only. Started through `start`, which gives it a console of its own and working
/// standard handles; a GUI process has none to pass on.
#[must_use]
pub fn power_shell_command(platform: Platform) -> Option<Launch> {
    (platform == Platform::Windows).then(|| Launch {
        hidden: true,
        ..Launch::plain("cmd.exe", &["/C", "start", "", "powershell.exe", "-NoExit"])
    })
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

/// Where `git-bash.exe` may be, best guess first: the installer's own record, then the
/// installation the `git` on PATH belongs to, then the default program folders.
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
    git_bash_candidates(&installer_records(), git.as_deref(), &program_dirs)
        .into_iter()
        .find(|path| path.is_file())
}

#[cfg(windows)]
fn installer_records() -> Vec<PathBuf> {
    [
        windows_registry::LOCAL_MACHINE,
        windows_registry::CURRENT_USER,
    ]
    .into_iter()
    .filter_map(|hive| hive.open(r"SOFTWARE\GitForWindows").ok())
    .filter_map(|key| key.get_string("InstallPath").ok())
    .map(PathBuf::from)
    .collect()
}

#[cfg(not(windows))]
fn installer_records() -> Vec<PathBuf> {
    Vec::new()
}

/// The paths for `SHFileOperationW`: absolute, `\`-separated, each ended by a NUL and the
/// list by another. A path that is not there is refused here, with its name, rather than
/// left to a shell error code.
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

/// The bin off Windows, where it is a command rather than a shell call.
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
            let files: Vec<String> = names
                .iter()
                .map(|name| format!("POSIX file \"{}\"", name.replace('"', "\\\"")))
                .collect();
            let script = format!(
                "tell application \"Finder\" to delete {{{}}}",
                files.join(", ")
            );
            Some(Launch::plain("osascript", &["-e", &script]))
        }
    }
}

/// Detached: the program outlives Cogit, and nothing waits for it. Explorer in particular
/// exits with 1 on success, so its status would say nothing anyway.
pub fn spawn(launch: &Launch, cwd: Option<&Path>) -> std::io::Result<()> {
    command_for(launch, cwd).spawn().map(drop)
}

/// For the few launches whose answer matters, such as moving files to the bin.
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
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}
