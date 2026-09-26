//! Opening a terminal at a repository. The choice of program is data, so it can be tested
//! for every platform without launching a single window (M3 T3.6).
//!
//! The caller spawns with the repository as the working directory. Only a program that
//! ignores its inherited directory is told the path, and then as one argument.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Terminal {
    /// Whatever the desktop calls its terminal.
    #[default]
    System,
    WindowsTerminal,
    PowerShell,
    Cmd,
    GitBash,
}

impl Terminal {
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::WindowsTerminal => "windowsTerminal",
            Self::PowerShell => "powerShell",
            Self::Cmd => "cmd",
            Self::GitBash => "gitBash",
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System default",
            Self::WindowsTerminal => "Windows Terminal",
            Self::PowerShell => "PowerShell",
            Self::Cmd => "Command Prompt",
            Self::GitBash => "Git Bash",
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        choices().into_iter().find(|kind| kind.id() == id)
    }
}

/// What this platform can actually offer. A choice nobody can run is not a choice.
#[must_use]
pub fn choices() -> Vec<Terminal> {
    if cfg!(windows) {
        vec![
            Terminal::System,
            Terminal::WindowsTerminal,
            Terminal::PowerShell,
            Terminal::Cmd,
            Terminal::GitBash,
        ]
    } else {
        vec![Terminal::System]
    }
}

#[must_use]
pub fn command_for(kind: Terminal, path: &str) -> (String, Vec<String>) {
    match kind {
        // `wt` starts in its own configured directory unless told otherwise.
        Terminal::WindowsTerminal => ("wt.exe".to_owned(), vec!["-d".to_owned(), path.to_owned()]),
        Terminal::PowerShell => ("powershell.exe".to_owned(), vec!["-NoExit".to_owned()]),
        Terminal::Cmd => ("cmd.exe".to_owned(), vec!["/K".to_owned()]),
        Terminal::GitBash => (
            bash_of(crate::desktop::find_git_bash().as_deref())
                .to_string_lossy()
                .into_owned(),
            vec!["--login".to_owned(), "-i".to_owned()],
        ),
        Terminal::System => system_terminal(path),
    }
}

/// `command_for` as the one desktop launcher starts it (`desktop::spawn`). A console
/// program started from a GUI goes through `cmd /C start`, hidden, as PowerShell of the
/// repository menu does (R-261): started bare it would share whatever console the app has.
#[must_use]
pub fn launch_for(kind: Terminal, path: &str) -> crate::desktop::Launch {
    let (program, args) = command_for(kind, path);
    let console = cfg!(windows)
        && matches!(
            kind,
            Terminal::PowerShell | Terminal::Cmd | Terminal::GitBash
        );
    if console {
        let mut relayed = vec!["/C".to_owned(), "start".to_owned(), String::new(), program];
        relayed.extend(args);
        return crate::desktop::Launch {
            program: "cmd.exe".to_owned(),
            args: relayed,
            verbatim: false,
            hidden: true,
        };
    }
    crate::desktop::Launch {
        hidden: cfg!(windows) && kind == Terminal::System,
        program,
        args,
        verbatim: false,
    }
}

/// The bash beside the `git-bash.exe` the Git Shell item found (registry, PATH, the
/// per-user install), else the usual install folder. A missing one fails visibly with the
/// spawn error rather than silently doing nothing.
#[must_use]
pub fn bash_of(git_bash: Option<&std::path::Path>) -> std::path::PathBuf {
    if let Some(root) = git_bash.and_then(std::path::Path::parent) {
        return root.join("bin").join("bash.exe");
    }
    const PREFERRED: &str = r"C:\Program Files\Git\bin\bash.exe";
    if std::path::Path::new(PREFERRED).is_file() {
        PREFERRED.into()
    } else {
        r"C:\Program Files (x86)\Git\bin\bash.exe".into()
    }
}

fn system_terminal(path: &str) -> (String, Vec<String>) {
    if cfg!(target_os = "macos") {
        (
            "open".to_owned(),
            vec!["-a".to_owned(), "Terminal".to_owned(), path.to_owned()],
        )
    } else if cfg!(windows) {
        // `start` needs an empty title first, or it takes the next quoted argument for one.
        (
            "cmd.exe".to_owned(),
            vec![
                "/C".to_owned(),
                "start".to_owned(),
                String::new(),
                "cmd.exe".to_owned(),
                "/K".to_owned(),
            ],
        )
    } else {
        ("x-terminal-emulator".to_owned(), Vec::new())
    }
}
