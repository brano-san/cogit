// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{RepoHandle, RepoState};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

#[test]
fn an_ordinary_repository_is_clean() {
    let f = test_fixtures::linear(2).unwrap();
    assert_eq!(open(&f).state().unwrap(), RepoState::Clean);
}

#[test]
fn a_repository_without_commits_reports_empty() {
    let f = test_fixtures::empty().unwrap();
    assert_eq!(open(&f).state().unwrap(), RepoState::Empty);
}

#[test]
fn a_bare_repository_reports_bare() {
    let f = test_fixtures::bare().unwrap();
    assert_eq!(open(&f).state().unwrap(), RepoState::Bare);
}

#[test]
fn a_detached_checkout_reports_its_commit() {
    let f = test_fixtures::detached_head().unwrap();
    match open(&f).state().unwrap() {
        RepoState::DetachedHead { oid } => assert_eq!(oid.len(), 40),
        other => panic!("expected a detached HEAD, got {other:?}"),
    }
}

#[test]
fn an_interrupted_merge_reports_merging() {
    let f = test_fixtures::conflicted().unwrap();
    assert_eq!(open(&f).state().unwrap(), RepoState::Merging);
}

#[test]
fn an_interrupted_rebase_reports_rebasing() {
    let f = test_fixtures::branched().unwrap();
    std::fs::create_dir_all(f.git_dir().join("rebase-merge")).unwrap();
    assert_eq!(open(&f).state().unwrap(), RepoState::Rebasing);
}

#[test]
fn an_interrupted_cherry_pick_reports_cherry_picking() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::write(f.git_dir().join("CHERRY_PICK_HEAD"), "0".repeat(40)).unwrap();
    assert_eq!(open(&f).state().unwrap(), RepoState::CherryPicking);
}

#[test]
fn an_interrupted_revert_reports_reverting() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::write(f.git_dir().join("REVERT_HEAD"), "0".repeat(40)).unwrap();
    assert_eq!(open(&f).state().unwrap(), RepoState::Reverting);
}

#[test]
fn a_bisect_in_progress_reports_bisecting() {
    let f = test_fixtures::linear(3).unwrap();
    std::fs::write(f.git_dir().join("BISECT_LOG"), "git bisect start\n").unwrap();
    assert_eq!(open(&f).state().unwrap(), RepoState::Bisecting);
}

#[test]
fn an_interrupted_operation_outranks_a_detached_head() {
    let f = test_fixtures::detached_head().unwrap();
    std::fs::write(f.git_dir().join("MERGE_HEAD"), "0".repeat(40)).unwrap();

    assert_eq!(
        open(&f).state().unwrap(),
        RepoState::Merging,
        "a half-finished merge is what the user has to deal with first"
    );
}

#[test]
fn a_locked_index_is_reported_with_the_path_to_the_lock() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.git_dir().join("index.lock"), "").unwrap();

    let lock = open(&f).index_lock();

    assert!(lock.is_some(), "a stale index.lock blocks every write");
    assert!(lock.unwrap().ends_with("index.lock"));
}

#[test]
fn no_lock_file_means_no_lock() {
    let f = test_fixtures::linear(1).unwrap();
    assert!(open(&f).index_lock().is_none());
}
