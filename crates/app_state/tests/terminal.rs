#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Which program opens a terminal at a folder. Pure, and parametrised by the platform, so
//! every choice is checked for Windows, macOS and Linux on any machine (M3 T3.6).
//!
//! The contract: the caller spawns the program with the repository as its working
//! directory. Only a program that ignores its inherited directory takes the path as an
//! argument, and then it is one argument, whatever spaces it holds.

use app_state::desktop::{Platform, native_path};
use app_state::terminal::{Terminal, choices, command_for, launch_for};
use std::path::Path;

const PATH: &str = "C:/work/repo";
const PLATFORMS: [Platform; 3] = [Platform::Windows, Platform::MacOs, Platform::Linux];

fn git_bash() -> Option<&'static Path> {
    Some(Path::new("D:/Tools/Git/git-bash.exe"))
}

fn every_choice() -> impl Iterator<Item = (Platform, Terminal)> {
    PLATFORMS.into_iter().flat_map(|platform| {
        choices(platform)
            .into_iter()
            .map(move |kind| (platform, kind))
    })
}

#[test]
fn every_choice_names_a_program() {
    for (platform, kind) in every_choice() {
        let (program, _) = command_for(platform, kind, PATH, git_bash());
        assert!(!program.is_empty(), "{platform:?} {kind:?} has no program");
    }
}

#[test]
fn no_choice_passes_the_path_where_a_command_is_expected() {
    // `bash -c <path>` would run the path as a command; so would `powershell <path>`.
    for (platform, kind) in every_choice() {
        let (_, args) = command_for(platform, kind, PATH, git_bash());
        for (before, arg) in args.iter().zip(args.iter().skip(1)) {
            if before == "-c" || before == "-Command" {
                assert_ne!(arg, PATH, "{platform:?} {kind:?} would execute the path");
            }
        }
    }
}

#[test]
fn the_choices_offered_are_distinct() {
    for platform in PLATFORMS {
        let all = choices(platform);
        let mut ids: Vec<&str> = all.iter().map(|kind| kind.id()).collect();
        ids.sort_unstable();
        let total = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), total, "two {platform:?} choices share an id");
    }
}

#[test]
fn an_id_round_trips_through_its_name() {
    for (platform, kind) in every_choice() {
        assert_eq!(Terminal::from_id(platform, kind.id()), Some(kind));
    }
}

#[test]
fn an_unknown_id_is_not_guessed_at() {
    for platform in PLATFORMS {
        assert_eq!(Terminal::from_id(platform, "nethack"), None);
    }
}

#[test]
fn a_path_given_as_an_argument_is_given_whole() {
    let spaced = "C:/my work/repo";
    for (platform, kind) in every_choice() {
        let (_, args) = command_for(platform, kind, spaced, git_bash());
        if args.iter().any(|arg| arg.contains("my work")) {
            assert!(
                args.iter().any(|arg| *arg == native_path(platform, spaced)),
                "{platform:?} {kind:?} split the path: {args:?}"
            );
        }
    }
}

#[test]
fn off_windows_the_desktop_terminal_is_the_only_choice() {
    for platform in [Platform::MacOs, Platform::Linux] {
        assert_eq!(choices(platform), [Terminal::System]);
        // A Windows choice saved in the settings falls back to the default.
        assert_eq!(Terminal::from_id(platform, "powerShell"), None);
    }
}

#[test]
fn macos_opens_terminal_app_at_the_folder() {
    let (program, args) = command_for(Platform::MacOs, Terminal::System, "/Users/me/repo", None);
    assert_eq!(program, "open");
    assert_eq!(args, ["-a", "Terminal", "/Users/me/repo"]);
    let launch = launch_for(Platform::MacOs, Terminal::System, "/Users/me/repo", None);
    assert_eq!((launch.program.as_str(), launch.hidden), ("open", false));
}

#[test]
fn linux_starts_the_terminal_emulator_in_the_inherited_directory() {
    let (program, args) = command_for(Platform::Linux, Terminal::System, "/home/me/repo", None);
    assert_eq!(program, "x-terminal-emulator");
    assert!(args.is_empty(), "the cwd carries it: {args:?}");
    let launch = launch_for(Platform::Linux, Terminal::System, "/home/me/repo", None);
    assert_eq!(
        (launch.program.as_str(), launch.hidden),
        ("x-terminal-emulator", false)
    );
}

mod windows {
    use super::{PATH, git_bash};
    use app_state::desktop::Platform;
    use app_state::terminal::{Terminal, bash_of, launch_for};

    fn command_for(kind: Terminal, path: &str) -> (String, Vec<String>) {
        app_state::terminal::command_for(Platform::Windows, kind, path, git_bash())
    }

    #[test]
    fn windows_terminal_is_told_the_directory_because_it_ignores_the_inherited_one() {
        let (program, args) = command_for(Terminal::WindowsTerminal, PATH);
        assert_eq!(program, "wt.exe");
        let at = args.iter().position(|arg| arg == "-d").expect("no -d");
        assert_eq!(args.get(at + 1).map(String::as_str), Some(r"C:\work\repo"));
    }

    #[test]
    fn powershell_stays_open_and_inherits_the_directory() {
        let (program, args) = command_for(Terminal::PowerShell, PATH);
        assert_eq!(program, "powershell.exe");
        assert!(args.iter().any(|arg| arg == "-NoExit"), "{args:?}");
        assert!(
            !args.iter().any(|arg| arg == PATH),
            "the cwd carries it: {args:?}"
        );
    }

    #[test]
    fn the_command_prompt_stays_open_too() {
        let (_, args) = command_for(Terminal::Cmd, PATH);
        assert!(args.iter().any(|arg| arg == "/K"), "{args:?}");
    }

    // Spawned bare from the Tauri layer, a console shell sat down in the console of `tauri
    // dev` instead of opening a window of its own, and took the app's handles (BE-008).
    #[test]
    fn a_console_shell_opens_in_a_console_of_its_own() {
        for kind in [Terminal::PowerShell, Terminal::Cmd, Terminal::GitBash] {
            let (program, args) = command_for(kind, PATH);
            let launch = launch_for(Platform::Windows, kind, PATH, git_bash());
            assert_eq!(launch.program, "cmd.exe", "{kind:?}");
            assert_eq!(launch.args[..3], ["/C", "start", ""], "{kind:?}");
            assert_eq!(launch.args[3], program, "{kind:?}");
            assert_eq!(launch.args[4..], args[..], "{kind:?}");
            assert!(launch.hidden, "the relaying cmd shows no window: {kind:?}");
        }
    }

    #[test]
    fn windows_terminal_and_the_system_choice_start_as_they_are() {
        let wt = launch_for(Platform::Windows, Terminal::WindowsTerminal, PATH, None);
        assert_eq!(wt.program, "wt.exe");
        let system = launch_for(Platform::Windows, Terminal::System, PATH, None);
        assert_eq!(system.args[..2], ["/C", "start"]);
        assert!(system.hidden);
    }

    #[test]
    fn git_bash_is_an_interactive_login_shell_and_nothing_else() {
        let (program, args) = command_for(Terminal::GitBash, PATH);
        assert_eq!(program, bash_of(git_bash()).to_string_lossy());
        assert_eq!(args, ["--login", "-i"]);
    }
}

#[test]
fn the_default_choice_is_one_of_the_offered_ones() {
    for platform in PLATFORMS {
        assert!(choices(platform).contains(&Terminal::default()));
    }
}

#[test]
fn every_choice_has_a_name_a_human_picked() {
    for (_, kind) in every_choice() {
        assert!(kind.label().len() > 2, "{kind:?} has no label");
    }
}

// Opening Git Bash knew only the two Program Files folders. Git installed for one user, in
// `%LOCALAPPDATA%\Programs\Git`, is found by the Git Shell menu item but not here: the
// terminal failed to start.
#[test]
fn git_bash_comes_from_the_install_that_was_found() {
    let found = std::path::Path::new("D:/Tools/Git/git-bash.exe");

    let bash = app_state::terminal::bash_of(Some(found));

    assert_eq!(bash, std::path::Path::new("D:/Tools/Git/bin/bash.exe"));
}
