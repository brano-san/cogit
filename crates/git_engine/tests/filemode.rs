// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::RepoHandle;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn staged_mode(f: &test_fixtures::Fixture, path: &str) -> String {
    f.git(&["ls-files", "--stage", "--", path])
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn a_mode_can_be_staged_without_the_content() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("script.sh", "#!/bin/sh\necho one\n").unwrap();
    f.git(&["add", "--", "script.sh"]).unwrap();
    f.commit_staged(1, "add a script").unwrap();
    f.write_file("script.sh", "#!/bin/sh\necho two\n").unwrap();

    open(&f).stage_mode("script.sh", true).unwrap();

    assert_eq!(staged_mode(&f, "script.sh"), "100755");
}

#[test]
fn staging_a_mode_leaves_the_content_change_unstaged() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("script.sh", "#!/bin/sh\necho one\n").unwrap();
    f.git(&["add", "--", "script.sh"]).unwrap();
    f.commit_staged(1, "add a script").unwrap();
    f.write_file("script.sh", "#!/bin/sh\necho two\n").unwrap();

    open(&f).stage_mode("script.sh", true).unwrap();

    let files = open(&f).worktree_files().unwrap();
    assert!(
        files.unstaged.iter().any(|entry| entry.path == "script.sh"),
        "{files:?}"
    );
}

#[test]
fn a_mode_can_be_taken_back_off() {
    let f = test_fixtures::filemode_change().unwrap();

    open(&f).stage_mode("script.sh", false).unwrap();

    assert_eq!(staged_mode(&f, "script.sh"), "100644");
}

#[test]
fn staging_the_mode_of_a_path_git_does_not_track_is_an_error() {
    let f = test_fixtures::linear(1).unwrap();
    assert!(open(&f).stage_mode("never-added.sh", true).is_err());
}

fn entry<'a>(list: &'a [git_engine::FileEntry], path: &str) -> &'a git_engine::FileEntry {
    list.iter()
        .find(|entry| entry.path == path)
        .unwrap_or_else(|| panic!("{path} is not listed: {list:?}"))
}

// Files marks a mode change `+x` / `−x`, and its "+x — Stage only the mode change" button
// skips a file without one: the status never said, so neither ever showed or did anything.
#[test]
fn a_staged_mode_change_is_marked_on_its_row() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("script.sh", "#!/bin/sh\necho one\n").unwrap();
    f.git(&["add", "--", "script.sh"]).unwrap();
    f.commit_staged(1, "add a script").unwrap();
    f.git(&["update-index", "--chmod=+x", "--", "script.sh"])
        .unwrap();

    let files = open(&f).worktree_files().unwrap();

    assert_eq!(
        entry(&files.staged, "script.sh").mode_change,
        Some(git_engine::FileMode::Executable)
    );
}

#[test]
fn a_file_whose_mode_stayed_has_no_mark() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("script.sh", "#!/bin/sh\necho one\n").unwrap();
    f.git(&["add", "--", "script.sh"]).unwrap();
    f.commit_staged(1, "add a script").unwrap();
    f.write_file("script.sh", "#!/bin/sh\necho two\n").unwrap();
    f.git(&["add", "--", "script.sh"]).unwrap();

    let files = open(&f).worktree_files().unwrap();

    assert_eq!(entry(&files.staged, "script.sh").mode_change, None);
}

#[cfg(unix)]
#[test]
fn an_unstaged_mode_change_is_marked_on_its_row() {
    use std::os::unix::fs::PermissionsExt as _;
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("script.sh", "#!/bin/sh\necho one\n").unwrap();
    f.git(&["add", "--", "script.sh"]).unwrap();
    f.commit_staged(1, "add a script").unwrap();
    let path = f.path().join("script.sh");
    let mut mode = std::fs::metadata(&path).unwrap().permissions();
    mode.set_mode(0o755);
    std::fs::set_permissions(&path, mode).unwrap();

    let files = open(&f).worktree_files().unwrap();

    assert_eq!(
        entry(&files.unstaged, "script.sh").mode_change,
        Some(git_engine::FileMode::Executable)
    );
}
