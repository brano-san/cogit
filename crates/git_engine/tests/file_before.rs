// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! "Open the file as it was before this commit" — the step back a blame view offers once
//! the user has found the commit that changed a line.

use git_engine::RepoHandle;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn text(bytes: Option<Vec<u8>>) -> Option<String> {
    bytes.map(|data| String::from_utf8_lossy(&data).into_owned())
}

#[test]
fn the_state_before_a_commit_is_its_parents_version() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("story.txt", "first\n").unwrap();
    f.git(&["add", "--", "story.txt"]).unwrap();
    f.commit_staged(10, "write it").unwrap();

    f.write_file("story.txt", "second\n").unwrap();
    f.git(&["add", "--", "story.txt"]).unwrap();
    let changed = f.commit_staged(11, "change it").unwrap();

    let before = open(&f).file_before(&changed, "story.txt").unwrap();

    assert_eq!(text(before).as_deref(), Some("first\n"));
}

#[test]
fn a_file_added_by_the_commit_has_no_state_before_it() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("fresh.txt", "brand new\n").unwrap();
    f.git(&["add", "--", "fresh.txt"]).unwrap();
    let added = f.commit_staged(10, "add it").unwrap();

    let before = open(&f).file_before(&added, "fresh.txt").unwrap();

    assert_eq!(before, None);
}

#[test]
fn a_file_deleted_by_the_commit_still_has_a_state_before_it() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("doomed.txt", "here for now\n").unwrap();
    f.git(&["add", "--", "doomed.txt"]).unwrap();
    f.commit_staged(10, "add it").unwrap();

    f.git(&["rm", "--", "doomed.txt"]).unwrap();
    let removed = f.commit_staged(11, "remove it").unwrap();

    let before = open(&f).file_before(&removed, "doomed.txt").unwrap();

    assert_eq!(text(before).as_deref(), Some("here for now\n"));
}

#[test]
fn the_state_before_the_root_commit_is_nothing_rather_than_an_error() {
    let f = test_fixtures::linear(1).unwrap();
    let root = f.git(&["rev-list", "--max-parents=0", "HEAD"]).unwrap();
    let root = root.trim();

    let before = open(&f).file_before(root, "file0.txt").unwrap();

    assert_eq!(before, None, "a root commit has no parent to look at");
}

#[test]
fn an_unknown_commit_is_a_typed_error() {
    let f = test_fixtures::linear(1).unwrap();

    let failed = open(&f).file_before("not-a-commit", "file0.txt");

    assert!(failed.is_err());
}

#[test]
fn a_path_that_never_existed_has_no_state_before_it() {
    let f = test_fixtures::linear(2).unwrap();
    let head = f.oid("HEAD").unwrap();

    let before = open(&f).file_before(&head, "never/existed.txt").unwrap();

    assert_eq!(before, None);
}
