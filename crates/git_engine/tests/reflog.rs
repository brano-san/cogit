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

#[test]
fn a_branch_is_restored_to_a_target_its_reflog_remembers() {
    let f = test_fixtures::linear(3).unwrap();
    let old = f.oid("HEAD~2").unwrap();
    let tip = f.oid("HEAD").unwrap();
    f.git(&["branch", "side", &old]).unwrap();
    f.git(&["branch", "-f", "side", &tip]).unwrap();
    let repo = open(&f);

    let entries = repo.reflog_for("side", 50).unwrap();
    assert_eq!(entries[0].selector, "side@{0}");
    assert_eq!(entries[1].oid, old);

    let before = repo.restore_branch("side", &old).unwrap();
    assert_eq!(before, tip);
    assert_eq!(f.oid("side").unwrap(), old);
}

#[test]
fn refs_outside_branches_and_tags_are_listed_as_other_refs() {
    let f = test_fixtures::linear(2).unwrap();
    let tip = f.oid("HEAD").unwrap();
    f.git(&["update-ref", "refs/pull/7/head", &tip]).unwrap();
    f.git(&["update-ref", "refs/notes/commits", &tip]).unwrap();
    f.git(&["stash", "list"]).unwrap();

    let mut found = open(&f).other_refs().unwrap();
    found.retain(|other| other.full_name.starts_with("refs/"));

    let names: Vec<&str> = found.iter().map(|o| o.full_name.as_str()).collect();
    assert_eq!(names, ["refs/notes/commits", "refs/pull/7/head"]);
    assert_eq!(found[1].oid, tip);
}

#[test]
fn standard_refs_and_the_stash_are_not_other_refs() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["tag", "v1"]).unwrap();
    std::fs::write(f.path().join("base.txt"), "changed\n").unwrap();
    f.git(&["stash"]).unwrap();

    let mut found = open(&f).other_refs().unwrap();
    found.retain(|other| other.full_name.starts_with("refs/"));

    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn rebase_head_is_listed_while_a_rebase_is_stopped() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["checkout", "-q", "dev"]).unwrap();
    std::fs::write(f.path().join("main-1.txt"), "dev clash\n").unwrap();
    f.git(&["add", "."]).unwrap();
    f.git(&["commit", "-q", "-m", "clash"]).unwrap();
    assert!(f.git(&["rebase", "main"]).is_err());

    let found = open(&f).other_refs().unwrap();

    assert!(
        found.iter().any(|o| o.full_name == "REBASE_HEAD"),
        "{found:?}"
    );
}

#[test]
fn orig_head_after_a_reset_is_listed_last_among_other_refs() {
    let f = test_fixtures::linear(2).unwrap();
    let tip = f.oid("HEAD").unwrap();
    f.git(&["reset", "--hard", "HEAD~1"]).unwrap();

    let found = open(&f).other_refs().unwrap();

    let last = found.last().unwrap();
    assert_eq!(
        (last.full_name.as_str(), last.oid.as_str()),
        ("ORIG_HEAD", tip.as_str())
    );
}

// Item 7: the view listed only what HEAD's reflog remembered. Everything below is
// unreachable from every ref (`git fsck --unreachable --no-reflogs`) and was missing.

fn lost_all(f: &test_fixtures::Fixture) -> Vec<String> {
    let mut oids: Vec<String> = open(f)
        .lost_commits(10_000)
        .unwrap()
        .into_iter()
        .map(|c| c.oid)
        .collect();
    oids.sort();
    oids
}

fn fsck_unreachable(f: &test_fixtures::Fixture) -> Vec<String> {
    let out = f
        .git(&["fsck", "--unreachable", "--no-reflogs", "--no-progress"])
        .unwrap();
    let mut oids: Vec<String> = out
        .lines()
        .filter_map(|line| line.strip_prefix("unreachable commit "))
        .map(str::to_owned)
        .collect();
    oids.sort();
    oids
}

#[test]
fn a_commit_no_reflog_remembers_is_lost() {
    let f = test_fixtures::linear(2).unwrap();
    let dangling = f
        .git(&["commit-tree", "HEAD^{tree}", "-p", "HEAD", "-m", "dangling"])
        .unwrap()
        .trim()
        .to_owned();

    assert_eq!(lost_all(&f), [dangling]);
}

#[test]
fn a_deleted_branch_loses_its_commits_with_its_reflog() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["switch", "-q", "-c", "topic"]).unwrap();
    f.commit_file(2, "a.txt", "a\n").unwrap();
    let tip = f.commit_file(3, "b.txt", "b\n").unwrap();
    f.git(&["switch", "-q", "main"]).unwrap();
    f.git(&["branch", "-D", "topic"]).unwrap();

    assert!(lost_all(&f).contains(&tip));
    assert_eq!(lost_all(&f), fsck_unreachable(&f));
}

#[test]
fn a_dropped_stash_is_lost_with_its_index_commit() {
    let f = test_fixtures::linear(2).unwrap();
    f.write_file("linear.txt", "changed\n").ok();
    f.write_file("new.txt", "untracked\n").unwrap();
    f.git(&["stash", "push", "-u", "-m", "wip"]).unwrap();
    let stash = f.oid("refs/stash").unwrap();
    f.git(&["stash", "drop"]).unwrap();

    let lost = lost_all(&f);

    assert!(lost.contains(&stash));
    assert_eq!(lost, fsck_unreachable(&f));
}

#[test]
fn a_kept_stash_is_not_lost() {
    let f = test_fixtures::with_stashes(2).unwrap();

    assert!(lost_all(&f).is_empty());
}

#[test]
fn the_head_of_another_worktree_keeps_its_commits() {
    let f = test_fixtures::with_worktree().unwrap();
    let wt = f
        .git(&["worktree", "list", "--porcelain"])
        .unwrap()
        .lines()
        .filter_map(|l| l.strip_prefix("worktree "))
        .find(|p| p.contains("linked"))
        .unwrap()
        .to_owned();
    f.git_in(std::path::Path::new(&wt), &["switch", "-q", "--detach"])
        .unwrap();
    let detached = f
        .git_in(
            std::path::Path::new(&wt),
            &["commit-tree", "HEAD^{tree}", "-p", "HEAD", "-m", "d"],
        )
        .unwrap()
        .trim()
        .to_owned();
    f.git_in(
        std::path::Path::new(&wt),
        &["update-ref", "--no-deref", "HEAD", &detached],
    )
    .unwrap();
    f.git(&["branch", "-D", "-f", "feature-wt"]).ok();

    let lost = lost_all(&f);

    assert!(!lost.contains(&detached), "{lost:?}");
    assert_eq!(lost, fsck_unreachable(&f));
}

#[test]
fn any_ref_keeps_its_commit_even_outside_branches_and_tags() {
    let f = test_fixtures::linear(2).unwrap();
    let dangling = f
        .git(&["commit-tree", "HEAD^{tree}", "-m", "kept"])
        .unwrap()
        .trim()
        .to_owned();
    f.git(&["update-ref", "refs/pull/7/head", &dangling])
        .unwrap();

    assert!(lost_all(&f).is_empty());
}

#[test]
fn more_lost_commits_than_the_limit_are_cut_after_filtering_newest_first() {
    let f = test_fixtures::linear(2).unwrap();
    for i in 0..15 {
        f.git_at(
            100 + i,
            &["commit-tree", "HEAD^{tree}", "-m", &format!("d{i}")],
        )
        .unwrap();
    }
    // Reachable reflog noise must not eat the limit: 16 entries, none of them lost.
    for _ in 0..8 {
        f.git(&["switch", "-q", "-c", "tmp"]).unwrap();
        f.git(&["switch", "-q", "main"]).unwrap();
        f.git(&["branch", "-q", "-D", "tmp"]).unwrap();
    }

    let rows = open(&f).lost_commits(10).unwrap();

    assert_eq!(rows.len(), 10);
    assert!(rows.windows(2).all(|w| w[0].timestamp >= w[1].timestamp));
    assert_eq!(open(&f).lost_commits(100).unwrap().len(), 15);
}

#[test]
fn the_lost_set_equals_fsck_on_a_messy_repository() {
    let f = test_fixtures::linear(3).unwrap();
    f.git(&["reset", "--hard", "HEAD~1"]).unwrap();
    f.git(&["commit", "--amend", "-q", "-m", "amended"])
        .unwrap();
    f.git(&["switch", "-q", "-c", "topic"]).unwrap();
    f.commit_file(9, "t.txt", "t\n").unwrap();
    f.git(&["switch", "-q", "main"]).unwrap();
    f.git(&["branch", "-D", "topic"]).unwrap();
    f.git(&["commit-tree", "HEAD^{tree}", "-m", "x"]).unwrap();

    assert_eq!(lost_all(&f), fsck_unreachable(&f));
}

// The object ids are listed again only when the object directories changed: the listing
// was the whole cost of a warm call (R-622).
#[test]
fn an_unchanged_object_store_is_not_listed_again_and_a_new_object_is_found() {
    let f = test_fixtures::linear(2).unwrap();
    let mut cache = Reachable::default();
    lost(&open(&f), &mut cache);
    // Objects younger than the filesystem's timestamp granularity cannot be trusted.
    std::thread::sleep(std::time::Duration::from_millis(2500));
    lost(&open(&f), &mut cache);
    let listed = cache.listings();

    lost(&open(&f), &mut cache);
    assert_eq!(cache.listings(), listed, "nothing changed");

    let dangling = f
        .git(&["commit-tree", "HEAD^{tree}", "-m", "late"])
        .unwrap()
        .trim()
        .to_owned();
    assert!(lost(&open(&f), &mut cache).contains(&dangling));
    assert_eq!(cache.listings(), listed + 1);
}

#[test]
fn a_new_pack_is_found_through_a_warm_cache() {
    let f = test_fixtures::linear(2).unwrap();
    let mut cache = Reachable::default();
    std::thread::sleep(std::time::Duration::from_millis(2500));
    lost(&open(&f), &mut cache);
    lost(&open(&f), &mut cache);
    let dangling = f
        .git(&["commit-tree", "HEAD^{tree}", "-m", "packed"])
        .unwrap()
        .trim()
        .to_owned();
    let packed = test_fixtures::git_command_in(f.path())
        .args(["pack-objects", "-q", ".git/objects/pack/extra"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write as _;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(format!("{dangling}\n").as_bytes())?;
            child.wait()
        });
    assert!(packed.is_ok());
    f.git(&["prune-packed"]).unwrap();

    assert!(lost(&open(&f), &mut cache).contains(&dangling));
}
