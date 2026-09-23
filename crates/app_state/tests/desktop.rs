#![allow(clippy::unwrap_used, clippy::expect_used)]

//! What the desktop is asked to do with a folder or a file. Pure, and parametrised by the
//! platform, so the Linux and macOS answers are checked on a Windows machine too.

use app_state::desktop::{
    Platform, file_uri, git_shell_command, native_path, open_command, power_shell_command,
    reveal_command, trash_command,
};
use std::path::{Path, PathBuf};

const REPO: &str = "D:/work/my repo";

#[test]
fn windows_paths_reach_explorer_with_backslashes() {
    // `explorer D:/work` opens Documents instead: it does not read forward slashes.
    assert_eq!(native_path(Platform::Windows, REPO), r"D:\work\my repo");
    assert_eq!(
        native_path(Platform::Linux, "/home/me/repo"),
        "/home/me/repo"
    );
}

#[test]
fn a_trailing_separator_is_dropped_but_a_drive_root_keeps_its_own() {
    assert_eq!(native_path(Platform::Windows, "D:/work/"), r"D:\work");
    assert_eq!(native_path(Platform::Windows, "D:/"), r"D:\");
    assert_eq!(native_path(Platform::Linux, "/"), "/");
}

#[test]
fn opening_a_folder_on_windows_hands_explorer_the_folder_itself() {
    let launch = open_command(Platform::Windows, REPO);
    assert_eq!(launch.program, "explorer.exe");
    assert_eq!(launch.args, [r#""D:\work\my repo""#]);
    assert!(launch.verbatim, "explorer parses its own command line");
}

#[test]
fn revealing_on_windows_selects_the_item_in_its_parent() {
    let launch = reveal_command(Platform::Windows, REPO);
    assert_eq!(launch.program, "explorer.exe");
    // One argument, quoted inside: the usual quoting would wrap `/select,` too, and
    // explorer then opens Documents.
    assert_eq!(launch.args, [r#"/select,"D:\work\my repo""#]);
    assert!(launch.verbatim);
}

#[test]
fn macos_opens_and_reveals_through_open() {
    let open = open_command(Platform::MacOs, "/Users/me/repo");
    assert_eq!(open.program, "open");
    assert_eq!(open.args, ["/Users/me/repo"]);
    let reveal = reveal_command(Platform::MacOs, "/Users/me/repo");
    assert_eq!(reveal.args, ["-R", "/Users/me/repo"]);
    assert!(!open.verbatim && !reveal.verbatim);
}

#[test]
fn linux_opens_with_xdg_open_and_reveals_through_the_file_manager_service() {
    let open = open_command(Platform::Linux, "/home/me/my repo");
    assert_eq!(open.program, "xdg-open");
    assert_eq!(open.args, ["/home/me/my repo"]);

    let reveal = reveal_command(Platform::Linux, "/home/me/my repo");
    assert_eq!(reveal.program, "dbus-send");
    assert!(
        reveal
            .args
            .contains(&"org.freedesktop.FileManager1.ShowItems".to_owned())
    );
    assert!(
        reveal
            .args
            .contains(&"array:string:file:///home/me/my%20repo".to_owned()),
        "{:?}",
        reveal.args
    );
}

#[test]
fn a_comma_in_a_linux_path_cannot_split_the_dbus_array() {
    assert_eq!(file_uri("/a,b/c d"), "file:///a%2Cb/c%20d");
}

#[test]
fn powershell_exists_only_on_windows_and_is_started_in_a_console_of_its_own() {
    let shell = power_shell_command(Platform::Windows).expect("offered on Windows");
    assert_eq!(shell.program, "cmd.exe");
    // `start` gives it a console and working handles; the relaying cmd stays invisible.
    assert_eq!(shell.args, ["/C", "start", "", "powershell.exe", "-NoExit"]);
    assert!(shell.hidden);
    assert!(power_shell_command(Platform::Linux).is_none());
    assert!(power_shell_command(Platform::MacOs).is_none());
}

#[test]
fn git_shell_starts_in_the_folder_it_was_asked_for() {
    let bash = Path::new(r"C:\Program Files\Git\git-bash.exe");
    let launch = git_shell_command(bash, Platform::Windows, REPO);
    assert_eq!(launch.program, bash.to_string_lossy());
    assert_eq!(launch.args, [r"--cd=D:\work\my repo"]);
    assert!(!launch.verbatim, "one argument, quoted the usual way");
}

#[test]
fn off_windows_the_bin_is_a_command_that_takes_every_path_whole() {
    let paths = [
        PathBuf::from("/home/me/a b.txt"),
        PathBuf::from("/home/me/c.txt"),
    ];
    let linux = trash_command(Platform::Linux, &paths).expect("gio");
    assert_eq!(linux.program, "gio");
    assert_eq!(
        linux.args,
        ["trash", "--", "/home/me/a b.txt", "/home/me/c.txt"]
    );

    let mac = trash_command(Platform::MacOs, &paths).expect("Finder");
    assert_eq!(mac.program, "osascript");
    assert!(
        mac.args[1].contains(r#"POSIX file "/home/me/a b.txt""#),
        "{:?}",
        mac.args
    );
    assert!(
        trash_command(Platform::Windows, &paths).is_none(),
        "the shell call does it"
    );
}

#[test]
fn the_current_platform_is_the_one_compiled_for() {
    let expected = if cfg!(windows) {
        Platform::Windows
    } else if cfg!(target_os = "macos") {
        Platform::MacOs
    } else {
        Platform::Linux
    };
    assert_eq!(Platform::current(), expected);
}

#[test]
fn each_platform_names_its_own_file_manager() {
    assert_eq!(Platform::Windows.file_manager(), "Explorer");
    assert_eq!(Platform::MacOs.file_manager(), "Finder");
    assert_eq!(Platform::Linux.file_manager(), "File Manager");
}

/// Windows paths only mean something to a Windows `Path`.
#[cfg(windows)]
mod windows {
    use app_state::desktop::{git_bash_candidates, trash_list};
    use std::path::{Path, PathBuf};

    #[test]
    fn git_bash_is_looked_for_where_the_installer_said_first() {
        let candidates = git_bash_candidates(
            &[PathBuf::from(r"E:\Tools\Git")],
            Some(Path::new(r"C:\Program Files\Git\cmd\git.exe")),
            &[PathBuf::from(r"C:\Program Files")],
        );
        assert_eq!(
            candidates.first(),
            Some(&PathBuf::from(r"E:\Tools\Git\git-bash.exe"))
        );
    }

    #[test]
    fn git_on_the_path_leads_to_its_own_installation() {
        for git in [
            r"C:\Git\cmd\git.exe",
            r"C:\Git\bin\git.exe",
            r"C:\Git\mingw64\bin\git.exe",
        ] {
            let candidates = git_bash_candidates(&[], Some(Path::new(git)), &[]);
            assert!(
                candidates.contains(&PathBuf::from(r"C:\Git\git-bash.exe")),
                "{git}: {candidates:?}"
            );
        }
    }

    #[test]
    fn the_program_folders_are_the_last_resort_and_nothing_is_listed_twice() {
        let candidates = git_bash_candidates(
            &[PathBuf::from(r"C:\Program Files\Git")],
            Some(Path::new(r"C:\Program Files\Git\cmd\git.exe")),
            &[
                PathBuf::from(r"C:\Program Files"),
                PathBuf::from(r"C:\Program Files (x86)"),
            ],
        );
        let default = PathBuf::from(r"C:\Program Files\Git\git-bash.exe");
        assert_eq!(
            candidates.iter().filter(|path| **path == default).count(),
            1
        );
        assert_eq!(
            candidates.last(),
            Some(&PathBuf::from(r"C:\Program Files (x86)\Git\git-bash.exe"))
        );
    }

    #[test]
    fn a_path_that_does_not_exist_is_refused_before_the_shell_is_asked() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("never-there.txt");
        let err = trash_list(&[missing]).unwrap_err();
        assert!(err.to_string().contains("never-there.txt"), "{err}");
    }

    #[test]
    fn the_list_is_absolute_backslashed_and_double_terminated() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a.txt"), dir.path().join("b.txt"));
        std::fs::write(&a, "a").unwrap();
        std::fs::write(&b, "b").unwrap();

        let list = trash_list(&[a, b]).unwrap();
        assert_eq!(&list[list.len() - 2..], [0, 0]);
        let text = String::from_utf16(&list[..list.len() - 2]).unwrap();
        let names: Vec<&str> = text.split('\0').collect();
        assert_eq!(names.len(), 2);
        assert!(
            names[0].ends_with(r"\a.txt") && !names[0].contains('/'),
            "{text}"
        );
        assert!(Path::new(names[1]).is_absolute());
    }
}
