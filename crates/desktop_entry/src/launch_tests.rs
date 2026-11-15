use super::*;

fn os(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

fn command(show: Show, paths: &[&str]) -> Option<Command> {
    Some(Command {
        show,
        paths: os(paths),
    })
}

#[test]
fn no_path_opens_the_window_as_before() {
    assert_eq!(parse(os(&[])), Ok(None));
    assert_eq!(parse(os(&["--some-flag", "-psn_0_42"])), Ok(None));
}

#[test]
fn bare_paths_open_repositories() {
    assert_eq!(parse(os(&["."])), Ok(command(Show::Repository, &["."])));
    assert_eq!(
        parse(os(&["a", "--x", "b"])),
        Ok(command(Show::Repository, &["a", "b"]))
    );
}

#[test]
fn a_folder_named_like_an_option_is_reached_after_a_separator() {
    assert_eq!(
        parse(os(&["--", "log"])),
        Ok(command(Show::Repository, &["log"]))
    );
    assert_eq!(
        parse(os(&["--", "-dir"])),
        Ok(command(Show::Repository, &["-dir"]))
    );
    assert_eq!(
        parse(os(&["./log"])),
        Ok(command(Show::Repository, &["./log"]))
    );
}

#[test]
fn relative_paths_resolve_against_the_working_directory() {
    let cwd = std::env::temp_dir().join("work").join("repo");
    let request = resolve(
        Command {
            show: Show::Repository,
            paths: os(&["src", "../other", "./x/./y"]),
        },
        &cwd,
    );
    assert_eq!(
        request.paths,
        [
            cwd.join("src"),
            std::env::temp_dir().join("work").join("other"),
            cwd.join("x").join("y"),
        ]
    );
}

#[test]
fn absolute_paths_are_kept() {
    let cwd = std::env::temp_dir();
    let elsewhere = std::env::temp_dir().join("elsewhere");
    let request = resolve(
        Command {
            show: Show::Repository,
            paths: vec![elsewhere.clone().into_os_string()],
        },
        &cwd.join("somewhere"),
    );
    assert_eq!(request.show, Show::Repository);
    assert_eq!(request.paths, [elsewhere]);
}

#[test]
fn parent_of_a_root_is_the_root() {
    let root: PathBuf = std::env::temp_dir().components().take(2).collect();
    assert_eq!(normalize(&root.join("..").join("a")), root.join("a"));
}

#[test]
fn explorers_drive_root_loses_its_stray_quote_on_windows_only() {
    assert_eq!(unquote(&"C:\"".into(), true), OsString::from("C:\\"));
    assert_eq!(
        unquote(&"D:\\My Folder\"".into(), true),
        OsString::from("D:\\My Folder\\")
    );
    assert_eq!(unquote(&"a\"".into(), false), OsString::from("a\""));
    assert_eq!(unquote(&"plain".into(), true), OsString::from("plain"));
}
