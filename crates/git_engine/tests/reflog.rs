// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{Reachable, RepoHandle};

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

fn lost(repo: &RepoHandle, cache: &mut Reachable) -> Vec<String> {
    repo.lost_commits_with(50, cache)
        .unwrap()
        .into_iter()
        .map(|c| c.oid)
        .collect()
}

#[test]
fn a_warm_cache_answers_like_a_fresh_count_after_a_new_commit() {
    let f = test_fixtures::linear(3).unwrap();
    let dropped = f.oid("HEAD").unwrap();
    f.git(&["reset", "--hard", "HEAD~1"]).unwrap();
    let mut cache = Reachable::default();
    assert!(lost(&open(&f), &mut cache).contains(&dropped));

    f.commit_file(10, "new.txt", "new\n").unwrap();

    let repo = open(&f);
    let fresh: Vec<String> = repo
        .lost_commits(50)
        .unwrap()
        .into_iter()
        .map(|c| c.oid)
        .collect();
    assert_eq!(lost(&repo, &mut cache), fresh);
    assert_eq!(
        cache.recounts(),
        1,
        "a commit on top is counted from the new tip alone"
    );
}

#[test]
fn a_branch_put_on_a_lost_commit_finds_it_through_a_warm_cache() {
    let f = test_fixtures::linear(3).unwrap();
    let dropped = f.oid("HEAD").unwrap();
    f.git(&["reset", "--hard", "HEAD~1"]).unwrap();
    let mut cache = Reachable::default();
    assert!(lost(&open(&f), &mut cache).contains(&dropped));

    f.git(&["branch", "rescue", &dropped]).unwrap();

    assert!(!lost(&open(&f), &mut cache).contains(&dropped));
}

#[test]
fn deleting_the_only_branch_to_a_commit_loses_it_through_a_warm_cache() {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["switch", "-c", "side"]).unwrap();
    f.commit_file(10, "side.txt", "side\n").unwrap();
    let side = f.oid("HEAD").unwrap();
    f.git(&["switch", "main"]).unwrap();
    let mut cache = Reachable::default();
    assert!(!lost(&open(&f), &mut cache).contains(&side));

    f.git(&["branch", "-D", "side"]).unwrap();

    assert!(lost(&open(&f), &mut cache).contains(&side));
    assert_eq!(
        cache.recounts(),
        2,
        "a vanished tip can shrink the set, so it is recounted"
    );
}

#[test]
fn an_unchanged_repository_is_answered_without_counting_again() {
    let f = test_fixtures::linear(3).unwrap();
    let mut cache = Reachable::default();

    lost(&open(&f), &mut cache);
    lost(&open(&f), &mut cache);

    assert_eq!(cache.recounts(), 1);
}

#[test]
fn entries_are_named_from_head_at_zero_and_carry_the_commit_time() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["switch", "dev"]).unwrap();
    f.git(&["switch", "-"]).unwrap();

    let entries = open(&f).reflog(50).unwrap();

    let names: Vec<&str> = entries.iter().map(|e| e.selector.as_str()).collect();
    assert_eq!(names[..2], ["HEAD@{0}", "HEAD@{1}"]);
    assert!(entries.iter().all(|e| e.timestamp > 0));
}

// A tag keeps its commit as surely as a branch does, but only branches and HEAD counted:
// deleting the branch under a tagged commit listed it among the lost.
#[test]
fn a_commit_only_a_tag_holds_is_not_lost() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["switch", "-q", "-c", "topic"]).unwrap();
    let tagged = f.commit_file(2, "topic.txt", "t\n").unwrap();
    f.git(&["tag", "v1"]).unwrap();
    f.git(&["switch", "-q", "main"]).unwrap();
    f.git(&["branch", "-D", "topic"]).unwrap();

    let lost = lost(&open(&f), &mut Reachable::default());

    assert!(!lost.contains(&tagged), "{lost:?}");
}

// A shallow clone: the parent of a newly fetched tip is not there. The fresh count skips
// what it cannot read; the count that only walks from the new tips gave up instead.
#[test]
fn a_tip_fetched_into_a_shallow_clone_does_not_break_a_warm_cache() {
    let upstream = test_fixtures::linear(3).unwrap();
    upstream.git(&["switch", "-q", "-c", "other"]).unwrap();
    upstream.commit_file(4, "other.txt", "o\n").unwrap();
    upstream.git(&["switch", "-q", "main"]).unwrap();
    let clone = tempfile::tempdir().unwrap();
    let url = format!(
        "file://{}",
        upstream.path().to_string_lossy().replace('\\', "/")
    );
    let target = clone.path().join("shallow");
    let status = test_fixtures::git_command_in(clone.path())
        .args(["clone", "-q", "--depth", "1", &url])
        .arg(&target)
        .status()
        .unwrap();
    assert!(status.success());
    let repo = RepoHandle::open(&target).unwrap();
    let mut cache = Reachable::default();
    repo.lost_commits_with(100, &mut cache).unwrap();

    let fetched = test_fixtures::git_command_in(&target)
        .args([
            "fetch",
            "-q",
            "--depth",
            "1",
            "origin",
            "other:refs/remotes/origin/other",
        ])
        .status()
        .unwrap();
    assert!(fetched.success());

    RepoHandle::open(&target)
        .unwrap()
        .lost_commits_with(100, &mut cache)
        .unwrap();
}

// The same, with the new tip's parent past the shallow boundary: the walk from the new tip
// read commits by id and failed on the missing parent, on every call until a reopen.
#[test]
fn a_tip_whose_parent_a_shallow_clone_lacks_does_not_break_a_warm_cache() {
    let upstream = test_fixtures::linear(3).unwrap();
    upstream
        .git(&["switch", "-q", "-c", "other", "HEAD~1"])
        .unwrap();
    upstream.commit_file(4, "other.txt", "o\n").unwrap();
    upstream.git(&["switch", "-q", "main"]).unwrap();
    let clone = tempfile::tempdir().unwrap();
    let url = format!(
        "file://{}",
        upstream.path().to_string_lossy().replace('\\', "/")
    );
    let target = clone.path().join("shallow");
    let status = test_fixtures::git_command_in(clone.path())
        .args(["clone", "-q", "--depth", "1", &url])
        .arg(&target)
        .status()
        .unwrap();
    assert!(status.success());
    let mut cache = Reachable::default();
    RepoHandle::open(&target)
        .unwrap()
        .lost_commits_with(100, &mut cache)
        .unwrap();

    let fetched = test_fixtures::git_command_in(&target)
        .args([
            "fetch",
            "-q",
            "--depth",
            "1",
            "origin",
            "other:refs/remotes/origin/other",
        ])
        .status()
        .unwrap();
    assert!(fetched.success());

    RepoHandle::open(&target)
        .unwrap()
        .lost_commits_with(100, &mut cache)
        .unwrap();
}
