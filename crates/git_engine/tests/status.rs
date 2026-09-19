// Integration tests are test code by definition, but `allow-unwrap-in-tests` in
// clippy.toml only covers the bodies of `#[test]` functions — helpers beside them are
// still linted. Panicking is how a test reports failure, so allow it for the file.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::RepoHandle;
use test_fixtures::Fixture;

fn status_of(f: &Fixture) -> git_engine::RepoStatus {
    RepoHandle::open(f.path()).unwrap().status().unwrap()
}

#[test]
fn a_clean_repository_reports_nothing() {
    let f = test_fixtures::linear(3).unwrap();
    let status = status_of(&f);
    assert_eq!(status.staged, 0);
    assert_eq!(status.unstaged, 0);
    assert_eq!(status.untracked, 0);
    assert!(status.is_clean());
}

#[test]
fn an_untracked_file_is_counted_separately() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("newcomer.txt", "hello\n").unwrap();

    let status = status_of(&f);
    assert_eq!(status.untracked, 1);
    assert_eq!(status.staged, 0);
    assert_eq!(status.unstaged, 0);
    assert!(!status.is_clean());
}

#[test]
fn a_staged_file_is_counted_as_staged() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("staged.txt", "hello\n").unwrap();
    f.git(&["add", "--", "staged.txt"]).unwrap();

    let status = status_of(&f);
    assert_eq!(status.staged, 1);
    assert_eq!(status.untracked, 0, "once staged it is no longer untracked");
}

#[test]
fn a_modified_tracked_file_is_counted_as_unstaged() {
    let f = test_fixtures::linear(2).unwrap();
    f.write_file("file0.txt", "changed after the commit\n")
        .unwrap();

    let status = status_of(&f);
    assert_eq!(status.unstaged, 1);
    assert_eq!(status.staged, 0);
}

#[test]
fn a_file_changed_both_before_and_after_staging_counts_twice() {
    let f = test_fixtures::linear(2).unwrap();
    f.write_file("file0.txt", "staged version\n").unwrap();
    f.git(&["add", "--", "file0.txt"]).unwrap();
    f.write_file("file0.txt", "and then changed again\n")
        .unwrap();

    let status = status_of(&f);
    assert_eq!(status.staged, 1);
    assert_eq!(status.unstaged, 1);
}

#[test]
fn a_deleted_file_is_counted_as_unstaged() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::remove_file(f.path().join("file0.txt")).unwrap();

    assert_eq!(status_of(&f).unstaged, 1);
}

#[test]
fn a_conflicted_file_is_reported_as_conflicted() {
    let f = test_fixtures::conflicted().unwrap();
    let status = status_of(&f);
    assert_eq!(status.conflicted, 1);
    assert!(!status.is_clean());
}

#[test]
fn an_empty_repository_has_a_status() {
    // INV-07: no commit yet is a normal state, not a failure.
    let f = test_fixtures::empty().unwrap();
    f.write_file("first.txt", "x\n").unwrap();

    let status = status_of(&f);
    assert_eq!(status.untracked, 1);
}

#[test]
fn a_bare_repository_reports_a_clean_empty_status() {
    let f = test_fixtures::bare().unwrap();
    assert!(status_of(&f).is_clean());
}

#[test]
fn ignored_files_are_not_counted() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file(".gitignore", "*.log\n").unwrap();
    f.git(&["add", "--", ".gitignore"]).unwrap();
    f.commit_staged(5, "add gitignore").unwrap();
    f.write_file("debug.log", "noise\n").unwrap();

    assert_eq!(status_of(&f).untracked, 0, "ignored files are not changes");
}
