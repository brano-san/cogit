// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{BranchKind, Head, RepoHandle};

#[test]
fn open_discovers_the_repository_from_its_root() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    assert_eq!(
        repo.root().canonicalize().unwrap(),
        f.path().canonicalize().unwrap()
    );
}

#[test]
fn open_discovers_the_repository_from_a_subdirectory() {
    let f = test_fixtures::unicode_paths().unwrap();
    let nested = f.path().join("каталог");
    let repo = RepoHandle::open(&nested).unwrap();
    assert_eq!(
        repo.root().canonicalize().unwrap(),
        f.path().canonicalize().unwrap()
    );
}

#[test]
fn open_rejects_a_directory_that_is_not_a_repository() {
    let dir = std::env::temp_dir().join("cogit-not-a-repo-test");
    std::fs::create_dir_all(&dir).unwrap();
    assert!(RepoHandle::open(&dir).is_err());
}

#[test]
fn head_reports_the_checked_out_branch() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    match repo.head().unwrap() {
        Head::Branch { name, oid } => {
            assert_eq!(name, "main");
            assert_eq!(oid, f.oid("HEAD").unwrap());
        }
        other => panic!("expected a branch, got {other:?}"),
    }
}

#[test]
fn head_reports_a_detached_state() {
    let f = test_fixtures::detached_head().unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    match repo.head().unwrap() {
        Head::Detached { oid } => assert_eq!(oid, f.oid("HEAD").unwrap()),
        other => panic!("expected a detached HEAD, got {other:?}"),
    }
}

#[test]
fn head_reports_an_unborn_branch_in_an_empty_repository() {
    let f = test_fixtures::empty().unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    match repo.head().unwrap() {
        Head::Unborn { name } => assert_eq!(name, "main"),
        other => panic!("expected an unborn HEAD, got {other:?}"),
    }
}

#[test]
fn branches_lists_every_local_branch() {
    let f = test_fixtures::branched().unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    let names: Vec<String> = repo
        .branches()
        .unwrap()
        .into_iter()
        .filter(|b| b.kind == BranchKind::Local)
        .map(|b| b.name)
        .collect();
    assert_eq!(names, vec!["dev".to_owned(), "main".to_owned()]);
}

#[test]
fn branches_come_back_sorted_by_name() {
    let f = test_fixtures::branched().unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    let names: Vec<String> = repo
        .branches()
        .unwrap()
        .into_iter()
        .map(|b| b.name)
        .collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
}

#[test]
fn branches_mark_the_checked_out_one() {
    let f = test_fixtures::branched().unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    let current: Vec<String> = repo
        .branches()
        .unwrap()
        .into_iter()
        .filter(|b| b.is_head)
        .map(|b| b.name)
        .collect();
    assert_eq!(current, vec!["main".to_owned()]);
}

#[test]
fn branches_carry_the_commit_they_point_at() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    let main = repo
        .branches()
        .unwrap()
        .into_iter()
        .find(|b| b.name == "main")
        .expect("main must exist");
    assert_eq!(main.oid, f.oid("main").unwrap());
}

#[test]
fn branches_of_an_empty_repository_are_empty_not_an_error() {
    let f = test_fixtures::empty().unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    assert!(repo.branches().unwrap().is_empty());
}

#[test]
fn a_bare_repository_can_still_be_read() {
    let f = test_fixtures::bare().unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    assert!(repo.is_bare());
    assert!(!repo.branches().unwrap().is_empty());
}

#[test]
fn reading_a_conflicted_repository_does_not_panic() {
    // INV-07: an interrupted merge is a normal state, not a reason to fall over.
    let f = test_fixtures::conflicted().unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    assert!(repo.head().is_ok());
    assert!(repo.branches().is_ok());
}

/// Regression guard for R-22: `gix` honours `GIT_INDEX_FILE`, so Cogit launched from
/// a hook would read a different repository than the user opened.
#[test]
fn an_inherited_git_index_file_is_ignored() {
    let f = test_fixtures::linear(3).unwrap();
    let other = test_fixtures::linear(1).unwrap();

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_probe-status"))
        .arg(f.path())
        .env("GIT_INDEX_FILE", other.git_dir().join("index"))
        .env("GIT_DIR", other.git_dir())
        .output()
        .unwrap();

    let out = String::from_utf8_lossy(&status.stdout);
    assert_eq!(
        out.trim(),
        "clean",
        "a foreign GIT_INDEX_FILE leaked in: {out}"
    );
}
