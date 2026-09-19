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
fn reads_a_file_as_it_was_in_a_commit() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);

    let blob = repo.blob_at("HEAD", "file2.txt").unwrap();

    assert_eq!(text(blob), "content 2\n");
}

#[test]
fn a_path_absent_from_the_tree_reads_as_nothing() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);

    assert!(repo.blob_at("HEAD", "never-existed.txt").unwrap().is_none());
}

#[test]
fn an_older_revision_shows_the_older_content() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("file0.txt"), "rewritten\n").unwrap();
    f.git(&["add", "--", "file0.txt"]).unwrap();
    f.commit_staged(1, "rewrite file0").unwrap();
    let repo = open(&f);

    assert_eq!(
        text(repo.blob_at("HEAD", "file0.txt").unwrap()),
        "rewritten\n"
    );
    assert_eq!(
        text(repo.blob_at("HEAD~1", "file0.txt").unwrap()),
        "content 0\n"
    );
}

#[test]
fn a_nested_path_is_found_through_its_directories() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::create_dir_all(f.path().join("src/deep")).unwrap();
    std::fs::write(f.path().join("src/deep/mod.rs"), "fn main() {}\n").unwrap();
    f.git(&["add", "--", "src/deep/mod.rs"]).unwrap();
    f.commit_staged(1, "add a nested file").unwrap();
    let repo = open(&f);

    assert_eq!(
        text(repo.blob_at("HEAD", "src/deep/mod.rs").unwrap()),
        "fn main() {}\n"
    );
}

#[test]
fn a_commit_is_compared_against_its_first_parent() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("file0.txt"), "after\n").unwrap();
    f.git(&["add", "--", "file0.txt"]).unwrap();
    f.commit_staged(1, "change it").unwrap();
    let repo = open(&f);

    let spec = DiffSpec::CommitVsParent {
        oid: f.oid("HEAD").unwrap(),
    };
    let (old, new) = repo.diff_sides(&spec, "file0.txt").unwrap();

    assert_eq!(text(old), "content 0\n");
    assert_eq!(text(new), "after\n");
}

#[test]
fn a_root_commit_has_nothing_on_the_parent_side() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    let spec = DiffSpec::CommitVsParent {
        oid: f.oid("HEAD").unwrap(),
    };
    let (old, new) = repo.diff_sides(&spec, "file0.txt").unwrap();

    assert!(old.is_none(), "a root commit adds every file");
    assert_eq!(text(new), "content 0\n");
}

#[test]
fn two_arbitrary_commits_can_be_compared() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);

    let spec = DiffSpec::CommitVsCommit {
        a: f.oid("HEAD~2").unwrap(),
        b: f.oid("HEAD").unwrap(),
    };
    let (old, new) = repo.diff_sides(&spec, "file2.txt").unwrap();

    assert!(old.is_none(), "file2 did not exist two commits ago");
    assert_eq!(text(new), "content 2\n");
}

#[test]
fn binary_content_comes_back_untouched() {
    let f = test_fixtures::linear(1).unwrap();
    let bytes: Vec<u8> = vec![0, 1, 2, 255, 0, 10];
    std::fs::write(f.path().join("blob.bin"), &bytes).unwrap();
    f.git(&["add", "--", "blob.bin"]).unwrap();
    f.commit_staged(1, "add a binary file").unwrap();
    let repo = open(&f);

    assert_eq!(repo.blob_at("HEAD", "blob.bin").unwrap().unwrap(), bytes);
}

#[test]
fn an_unknown_revision_is_a_typed_error() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    assert!(repo.blob_at("no-such-rev", "file0.txt").is_err());
}

#[test]
fn a_directory_is_not_mistaken_for_a_file() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::create_dir_all(f.path().join("src")).unwrap();
    std::fs::write(f.path().join("src/one.txt"), "one\n").unwrap();
    f.git(&["add", "--", "src/one.txt"]).unwrap();
    f.commit_staged(1, "add src").unwrap();
    let repo = open(&f);

    assert!(repo.blob_at("HEAD", "src").unwrap().is_none());
}
