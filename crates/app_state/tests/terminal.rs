#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Which program opens a terminal at a folder. Pure so it can be checked for every choice
//! on every platform, rather than by launching windows (M3 T3.6).
//!
//! The contract: the caller spawns the program with the repository as its working
//! directory. Only a program that ignores its inherited directory takes the path as an
//! argument, and then it is one argument, whatever spaces it holds.

use app_state::terminal::{Terminal, choices, command_for};

const PATH: &str = "C:/work/repo";

#[test]
fn every_choice_names_a_program() {
    for kind in choices() {
        let (program, _) = command_for(kind, PATH);
        assert!(!program.is_empty(), "{kind:?} has no program");
    }
}

#[test]
fn no_choice_passes_the_path_where_a_command_is_expected() {
    // `bash -c <path>` would run the path as a command; so would `powershell <path>`.
    for kind in choices() {
        let (_, args) = command_for(kind, PATH);
        for (before, arg) in args.iter().zip(args.iter().skip(1)) {
            if before == "-c" || before == "-Command" {
                assert_ne!(arg, PATH, "{kind:?} would execute the path");
            }
        }
    }
}

#[test]
fn the_choices_offered_are_distinct() {
    let all = choices();
    let mut ids: Vec<&str> = all.iter().map(|kind| kind.id()).collect();
    ids.sort_unstable();
    let total = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), total, "two choices share an id");
}

#[test]
fn an_id_round_trips_through_its_name() {
    for kind in choices() {
        assert_eq!(Terminal::from_id(kind.id()), Some(kind));
    }
}

#[test]
fn an_unknown_id_is_not_guessed_at() {
    assert_eq!(Terminal::from_id("nethack"), None);
}

#[test]
fn a_path_given_as_an_argument_is_given_whole() {
    let spaced = "C:/my work/repo";
    for kind in choices() {
        let (_, args) = command_for(kind, spaced);
        if args.iter().any(|arg| arg.contains("my work")) {
            assert!(
                args.iter().any(|arg| arg == spaced),
                "{kind:?} split the path: {args:?}"
            );
        }
    }
}

#[cfg(windows)]
mod windows {
    use super::{PATH, command_for};
    use app_state::terminal::Terminal;

    #[test]
    fn windows_terminal_is_told_the_directory_because_it_ignores_the_inherited_one() {
        let (program, args) = command_for(Terminal::WindowsTerminal, PATH);
        assert_eq!(program, "wt.exe");
        let at = args.iter().position(|arg| arg == "-d").expect("no -d");
        assert_eq!(args.get(at + 1).map(String::as_str), Some(PATH));
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

    #[test]
    fn git_bash_is_an_interactive_login_shell_and_nothing_else() {
        let (program, args) = command_for(Terminal::GitBash, PATH);
        assert!(program.ends_with("bash.exe"), "{program}");
        assert_eq!(args, ["--login", "-i"]);
    }
}

#[test]
fn the_default_choice_is_one_of_the_offered_ones() {
    assert!(choices().contains(&Terminal::default()));
}

#[test]
fn every_choice_has_a_name_a_human_picked() {
    for kind in choices() {
        assert!(kind.label().len() > 2, "{kind:?} has no label");
    }
}
