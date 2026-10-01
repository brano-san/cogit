// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, RepoId};
use git_engine::GitError;

fn open(f: &test_fixtures::Fixture) -> (AppState, RepoId) {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    (state, repo)
}

#[test]
fn a_fresh_repository_has_no_orig_head_and_the_undo_says_why() {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["update-ref", "-d", "ORIG_HEAD"]).unwrap();
    let (state, repo) = open(&f);

    assert!(state.undo_rewrite_info(repo).unwrap().is_none());
    let err = state.undo_rewrite(repo).unwrap_err();

    assert!(
        matches!(err, GitError::InvalidState(ref m) if m.contains("ORIG_HEAD")),
        "{err:?}"
    );
}

#[test]
fn a_merge_is_undone_to_where_the_branch_was() {
    let f = test_fixtures::branched().unwrap();
    let before = f.oid("HEAD").unwrap();
    f.git(&["merge", "--no-ff", "-m", "merge dev", "dev"])
        .unwrap();
    let (state, repo) = open(&f);

    let info = state.undo_rewrite_info(repo).unwrap().unwrap();
    assert_eq!(info.orig, before);
    assert!(!info.dirty);
    state.undo_rewrite(repo).unwrap();

    assert_eq!(f.oid("HEAD").unwrap(), before);
}

#[test]
fn a_hard_reset_is_undone() {
    let f = test_fixtures::linear(3).unwrap();
    let tip = f.oid("HEAD").unwrap();
    f.git(&["reset", "--hard", "HEAD~2"]).unwrap();
    let (state, repo) = open(&f);

    state.undo_rewrite(repo).unwrap();

    assert_eq!(f.oid("HEAD").unwrap(), tip);
}

#[test]
fn a_rebase_is_undone() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["checkout", "-q", "dev"]).unwrap();
    let before = f.oid("HEAD").unwrap();
    f.git(&["rebase", "main"]).unwrap();
    assert_ne!(f.oid("HEAD").unwrap(), before);
    let (state, repo) = open(&f);

    state.undo_rewrite(repo).unwrap();

    assert_eq!(f.oid("HEAD").unwrap(), before);
}

#[test]
fn local_changes_survive_the_undo_and_are_reported_before_it() {
    let f = test_fixtures::branched().unwrap();
    let before = f.oid("HEAD").unwrap();
    f.git(&["merge", "--no-ff", "-m", "merge dev", "dev"])
        .unwrap();
    std::fs::write(f.path().join("base.txt"), "staged edit\n").unwrap();
    f.git(&["add", "base.txt"]).unwrap();
    std::fs::write(f.path().join("scratch.txt"), "untracked\n").unwrap();
    let (state, repo) = open(&f);

    assert!(state.undo_rewrite_info(repo).unwrap().unwrap().dirty);
    state.undo_rewrite(repo).unwrap();

    assert_eq!(f.oid("HEAD").unwrap(), before);
    let read = |name: &str| std::fs::read_to_string(f.path().join(name)).unwrap();
    assert_eq!(read("base.txt"), "staged edit\n");
    assert_eq!(read("scratch.txt"), "untracked\n");
    // Staged edits may come back unstaged, but are never lost.
    assert!(
        f.git(&["status", "--porcelain"])
            .unwrap()
            .contains("base.txt")
    );
}

#[test]
fn an_undo_that_would_overwrite_local_work_changes_nothing() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["merge", "--no-ff", "-m", "merge dev", "dev"])
        .unwrap();
    let merged = f.oid("HEAD").unwrap();
    std::fs::write(f.path().join("dev-1.txt"), "my edit\n").unwrap();
    let (state, repo) = open(&f);

    assert!(state.undo_rewrite(repo).is_err());

    assert_eq!(f.oid("HEAD").unwrap(), merged);
    assert_eq!(
        std::fs::read_to_string(f.path().join("dev-1.txt")).unwrap(),
        "my edit\n"
    );
}

#[test]
fn the_undo_is_in_the_journal_and_the_journal_undo_still_works() {
    let f = test_fixtures::linear(3).unwrap();
    let tip = f.oid("HEAD").unwrap();
    f.git(&["reset", "--hard", "HEAD~1"]).unwrap();
    let (state, repo) = open(&f);

    state.undo_rewrite(repo).unwrap();
    assert_eq!(f.oid("HEAD").unwrap(), tip);
    state.undo_last(repo).unwrap();

    assert_ne!(f.oid("HEAD").unwrap(), tip);
}

#[test]
fn compare_before_and_after_rewrite_reads_orig_head_without_the_list() {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["commit", "-q", "--amend", "-m", "reworded"])
        .unwrap();
    let (state, repo) = open(&f);

    let out = state.range_diff(repo, "ORIG_HEAD", "HEAD").unwrap();

    assert!(out.stdout.contains("reworded"), "{}", out.stdout);
}
