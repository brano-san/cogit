// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{DiffSpec, RepoHandle};

fn open(fixture: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(fixture.path()).unwrap()
}

fn text(blob: Option<Vec<u8>>) -> String {
    String::from_utf8(blob.expect("expected the file to exist on this side")).unwrap()
}

#[test]
fn an_unstaged_edit_compares_the_index_with_the_file_on_disk() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("file0.txt"), "edited on disk\n").unwrap();
    let repo = open(&f);

    let (old, new) = repo
        .diff_sides(&DiffSpec::WorkTreeVsIndex, "file0.txt")
        .unwrap();

    assert_eq!(text(old), "content 0\n");
    assert_eq!(text(new), "edited on disk\n");
}

#[test]
fn a_staged_edit_compares_head_with_the_index() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("file0.txt"), "staged\n").unwrap();
    f.git(&["add", "--", "file0.txt"]).unwrap();
    let repo = open(&f);

    let (old, new) = repo
        .diff_sides(&DiffSpec::IndexVsHead, "file0.txt")
        .unwrap();

    assert_eq!(text(old), "content 0\n");
    assert_eq!(text(new), "staged\n");
}

#[test]
fn an_untracked_file_has_nothing_on_the_index_side() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("fresh.txt"), "brand new\n").unwrap();
    let repo = open(&f);

    let (old, new) = repo
        .diff_sides(&DiffSpec::WorkTreeVsIndex, "fresh.txt")
        .unwrap();

    assert!(old.is_none());
    assert_eq!(text(new), "brand new\n");
}

#[test]
fn a_file_deleted_from_disk_has_nothing_on_the_worktree_side() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::remove_file(f.path().join("file0.txt")).unwrap();
    let repo = open(&f);

    let (old, new) = repo
        .diff_sides(&DiffSpec::WorkTreeVsIndex, "file0.txt")
        .unwrap();

    assert_eq!(text(old), "content 0\n");
    assert!(new.is_none());
}

#[test]
fn a_staged_addition_has_nothing_on_the_head_side() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("fresh.txt"), "new\n").unwrap();
    f.git(&["add", "--", "fresh.txt"]).unwrap();
    let repo = open(&f);

    let (old, new) = repo
        .diff_sides(&DiffSpec::IndexVsHead, "fresh.txt")
        .unwrap();

    assert!(old.is_none());
    assert_eq!(text(new), "new\n");
}

#[test]
fn an_unborn_head_makes_everything_staged_an_addition() {
    let f = test_fixtures::empty().unwrap();
    std::fs::write(f.path().join("first.txt"), "content\n").unwrap();
    f.git(&["add", "--", "first.txt"]).unwrap();
    let repo = open(&f);

    let (old, new) = repo
        .diff_sides(&DiffSpec::IndexVsHead, "first.txt")
        .unwrap();

    assert!(old.is_none(), "INV-07: no HEAD is a normal state");
    assert_eq!(text(new), "content\n");
}

#[test]
fn the_worktree_side_reads_bytes_from_disk_untouched() {
    let f = test_fixtures::linear(1).unwrap();
    let bytes: Vec<u8> = vec![0, 1, 2, 255, 10];
    std::fs::write(f.path().join("blob.bin"), &bytes).unwrap();
    let repo = open(&f);

    let (_, new) = repo
        .diff_sides(&DiffSpec::WorkTreeVsIndex, "blob.bin")
        .unwrap();

    assert_eq!(new.unwrap(), bytes);
}

#[test]
fn a_nested_worktree_path_is_found() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::create_dir_all(f.path().join("src/deep")).unwrap();
    std::fs::write(f.path().join("src/deep/new.rs"), "fn main() {}\n").unwrap();
    let repo = open(&f);

    let (_, new) = repo
        .diff_sides(&DiffSpec::WorkTreeVsIndex, "src/deep/new.rs")
        .unwrap();

    assert_eq!(text(new), "fn main() {}\n");
}

#[test]
fn a_bare_repository_has_no_worktree_side() {
    let f = test_fixtures::bare().unwrap();
    let repo = open(&f);

    let (_, new) = repo
        .diff_sides(&DiffSpec::WorkTreeVsIndex, "anything.txt")
        .unwrap();

    assert!(new.is_none());
}

#[test]
fn a_past_commit_compares_with_the_file_on_disk_as_it_is_now() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::write(f.path().join("file0.txt"), "edited since\n").unwrap();
    let first = f.oid("HEAD~1").unwrap();
    let repo = open(&f);

    let (old, new) = repo
        .diff_sides(&DiffSpec::CommitVsWorkTree { oid: first }, "file0.txt")
        .unwrap();

    assert_eq!(text(old), "content 0\n");
    assert_eq!(text(new), "edited since\n");
}

#[test]
fn a_file_gone_from_disk_compares_with_nothing() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::remove_file(f.path().join("file1.txt")).unwrap();
    let repo = open(&f);

    let (old, new) = repo
        .diff_sides(
            &DiffSpec::CommitVsWorkTree {
                oid: "HEAD".to_owned(),
            },
            "file1.txt",
        )
        .unwrap();

    assert_eq!(text(old), "content 1\n");
    assert!(new.is_none());
}
