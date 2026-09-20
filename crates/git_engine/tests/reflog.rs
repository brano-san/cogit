// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::RepoHandle;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

#[test]
fn the_reflog_lists_what_head_has_done() {
    let f = test_fixtures::linear(3).unwrap();

    let entries = open(&f).reflog(50).unwrap();

    assert!(!entries.is_empty());
    assert!(entries.iter().all(|e| e.oid.len() == 40));
}

#[test]
fn the_reflog_is_newest_first() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);

    let entries = repo.reflog(50).unwrap();

    assert_eq!(entries[0].oid, f.oid("HEAD").unwrap());
}

#[test]
fn each_entry_says_what_moved_head() {
    let f = test_fixtures::branched().unwrap();
    // The shape fixtures are built with `fast-import`, which writes no reflog at all. A
    // real switch is what this test is about, so it performs one (R-56).
    f.git(&["switch", "dev"]).unwrap();

    let entries = open(&f).reflog(50).unwrap();

    assert!(
        entries.iter().any(|e| e.action.contains("checkout")),
        "switching branches must be visible: {:?}",
        entries.iter().map(|e| &e.action).collect::<Vec<_>>()
    );
}

#[test]
fn the_limit_is_respected() {
    let f = test_fixtures::linear(6).unwrap();

    assert!(open(&f).reflog(2).unwrap().len() <= 2);
}

#[test]
fn a_repository_without_commits_has_an_empty_reflog() {
    let f = test_fixtures::empty().unwrap();

    assert!(open(&f).reflog(50).unwrap().is_empty());
}

#[test]
fn a_commit_dropped_by_reset_is_still_reachable_through_the_reflog() {
    let f = test_fixtures::linear(3).unwrap();
    let lost = f.oid("HEAD").unwrap();
    f.git(&["reset", "--hard", "HEAD~1"]).unwrap();
    let repo = open(&f);

    let entries = repo.reflog(50).unwrap();

    assert!(
        entries.iter().any(|e| e.oid == lost),
        "the whole point of the reflog is finding what was dropped"
    );
}

#[test]
fn lost_commits_are_those_no_branch_or_tag_can_reach() {
    let f = test_fixtures::linear(3).unwrap();
    let lost = f.oid("HEAD").unwrap();
    f.git(&["reset", "--hard", "HEAD~1"]).unwrap();
    let repo = open(&f);

    let commits = repo.lost_commits(50).unwrap();

    assert!(commits.iter().any(|c| c.oid == lost), "{commits:?}");
}

#[test]
fn a_reachable_commit_is_not_reported_as_lost() {
    let f = test_fixtures::linear(3).unwrap();
    let head = f.oid("HEAD").unwrap();

    let commits = open(&f).lost_commits(50).unwrap();

    assert!(!commits.iter().any(|c| c.oid == head));
}

#[test]
fn a_clean_repository_has_nothing_lost() {
    let f = test_fixtures::linear(3).unwrap();

    assert!(open(&f).lost_commits(50).unwrap().is_empty());
}

#[test]
fn a_lost_commit_keeps_its_summary_so_it_can_be_recognised() {
    let f = test_fixtures::linear(3).unwrap();
    f.git(&["reset", "--hard", "HEAD~1"]).unwrap();
    let repo = open(&f);

    let commits = repo.lost_commits(50).unwrap();

    assert_eq!(commits[0].summary, "commit 2");
}
