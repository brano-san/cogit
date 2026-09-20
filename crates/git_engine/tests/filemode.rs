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
