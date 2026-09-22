// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{CommitRow, RepoHandle};
use test_fixtures::Fixture;

fn walk(repo: &RepoHandle, chunk_size: usize) -> (Vec<CommitRow>, Vec<usize>) {
    let mut all = Vec::new();
    let mut sizes = Vec::new();
    repo.stream_commits(chunk_size, |chunk| {
        sizes.push(chunk.len());
        all.extend(chunk);
        true
    })
    .unwrap();
    (all, sizes)
}

fn open(f: &Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

#[test]
fn streams_every_commit_of_a_linear_history() {
    let f = test_fixtures::linear(5).unwrap();
    let (commits, _) = walk(&open(&f), 100);
    assert_eq!(commits.len(), 5);
}

#[test]
fn commits_arrive_newest_first() {
    let f = test_fixtures::linear(4).unwrap();
    let (commits, _) = walk(&open(&f), 100);
    assert_eq!(commits[0].oid, f.oid("HEAD").unwrap());
    assert_eq!(commits[3].oid, f.oid("HEAD~3").unwrap());
}

#[test]
fn each_commit_carries_its_parent() {
    let f = test_fixtures::linear(3).unwrap();
    let (commits, _) = walk(&open(&f), 100);
    assert_eq!(commits[0].parents, vec![f.oid("HEAD~1").unwrap()]);
    assert!(commits[2].parents.is_empty(), "the root has no parent");
}

#[test]
fn a_merge_reports_both_parents_in_order() {
    // Order matters: the first parent is the mainline, and the lane algorithm relies
    // on it to keep history vertical.
    let f = test_fixtures::diamond().unwrap();
    let (commits, _) = walk(&open(&f), 100);
    let merge = &commits[0];
    assert_eq!(merge.parents.len(), 2);
    assert_eq!(merge.parents[0], f.oid("HEAD^1").unwrap());
    assert_eq!(merge.parents[1], f.oid("HEAD^2").unwrap());
}

#[test]
fn commits_carry_author_and_summary() {
    let f = test_fixtures::linear(1).unwrap();
    let (commits, _) = walk(&open(&f), 100);
    assert_eq!(commits[0].author_name, test_fixtures::AUTHOR_NAME);
    assert_eq!(commits[0].author_email, test_fixtures::AUTHOR_EMAIL);
    assert_eq!(commits[0].summary, "commit 0");
    assert_eq!(commits[0].timestamp, test_fixtures::BASE_TIMESTAMP);
}

#[test]
fn the_summary_is_only_the_first_line() {
    let f = Fixture::init().unwrap();
    f.write_file("a.txt", "x\n").unwrap();
    f.git(&["add", "-A"]).unwrap();
    f.git_at(
        0,
        &[
            "commit",
            "-m",
            "short title\n\nA longer body that must not leak.",
        ],
    )
    .unwrap();

    let (commits, _) = walk(&open(&f), 100);
    assert_eq!(commits[0].summary, "short title");
}

#[test]
fn history_of_branches_not_reachable_from_head_is_included() {
    let f = test_fixtures::branched().unwrap();
    let (commits, _) = walk(&open(&f), 100);
    let dev_tip = f.oid("dev").unwrap();
    assert!(
        commits.iter().any(|c| c.oid == dev_tip),
        "commits only on dev must still be walked"
    );
}

#[test]
fn chunks_are_no_larger_than_requested() {
    let f = test_fixtures::linear(7).unwrap();
    let (commits, sizes) = walk(&open(&f), 3);
    assert_eq!(commits.len(), 7);
    assert!(sizes.iter().all(|n| *n <= 3), "chunk sizes were {sizes:?}");
    assert_eq!(sizes.iter().sum::<usize>(), 7);
}

#[test]
fn returning_false_stops_the_walk_early() {
    // Two more than the chunk: enough to prove the walk stopped, cheap enough to run often.
    let f = test_fixtures::linear(12).unwrap();
    let repo = open(&f);
    let mut seen = 0;
    repo.stream_commits(10, |chunk| {
        seen += chunk.len();
        false
    })
    .unwrap();
    assert_eq!(seen, 10, "the walk must stop after the first refused chunk");
}

#[test]
fn an_empty_repository_streams_nothing() {
    let f = test_fixtures::empty().unwrap();
    let (commits, sizes) = walk(&open(&f), 10);
    assert!(commits.is_empty());
    assert!(sizes.is_empty(), "no chunk should be sent at all");
}

#[test]
fn a_repository_with_two_roots_walks_both() {
    let f = test_fixtures::two_roots().unwrap();
    let (commits, _) = walk(&open(&f), 100);
    let roots = commits.iter().filter(|c| c.parents.is_empty()).count();
    assert_eq!(roots, 2);
}

#[test]
fn every_commit_appears_exactly_once() {
    let f = test_fixtures::diamond().unwrap();
    let (commits, _) = walk(&open(&f), 100);
    let mut oids: Vec<&str> = commits.iter().map(|c| c.oid.as_str()).collect();
    let total = oids.len();
    oids.sort_unstable();
    oids.dedup();
    assert_eq!(oids.len(), total, "a commit was emitted more than once");
}

// --- the order the graph is drawn in ------------------------------------------------

/// Two branches whose commits alternate in time: `f2 m2 f1 m1` by date.
fn interleaved() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(1).unwrap();
    f.commit_file(
        10, "m1.txt", "one
",
    )
    .unwrap();
    f.git(&["switch", "-c", "feature"]).unwrap();
    f.commit_file(
        20, "f1.txt", "one
",
    )
    .unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.commit_file(
        30, "m2.txt", "two
",
    )
    .unwrap();
    f.git(&["switch", "feature"]).unwrap();
    f.commit_file(
        40, "f2.txt", "two
",
    )
    .unwrap();
    f.git(&["switch", "main"]).unwrap();
    f
}

fn walked(repo: &RepoHandle) -> Vec<String> {
    let query = git_engine::CommitQuery {
        visible_refs: None,
        ..Default::default()
    };
    let mut seen = Vec::new();
    repo.search_commits(&query, 50, |rows| {
        seen.extend(rows.into_iter().map(|row| row.summary));
        true
    })
    .unwrap();
    seen
}

/// `git log --date-order`: lines that lived side by side are drawn side by side, which
/// keeps every edge as short as the history allows (doc/12-risks.md, R-162).
#[test]
fn lines_that_lived_at_the_same_time_are_read_in_date_order() {
    let f = interleaved();
    let repo = RepoHandle::open(f.path()).unwrap();

    let order = walked(&repo);

    assert_eq!(
        order[..4],
        ["commit 40", "commit 30", "commit 20", "commit 10"],
        "{order:?}"
    );
}

/// The clock of whoever made a commit is not the history. A child dated before its
/// parent comes first anyway; by date alone the parent would sit above it and the line
/// waiting for it would run to the bottom of the graph.
#[test]
fn a_child_committed_with_a_clock_behind_its_parent_still_comes_first() {
    let f = test_fixtures::linear(1).unwrap();
    f.commit_file(50, "a.txt", "a\n").unwrap();
    f.commit_file(40, "b.txt", "b\n").unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    let order = walked(&repo);

    let at = |what: &str| order.iter().position(|row| row == what).unwrap();
    assert!(at("commit 40") < at("commit 50"), "{order:?}");
}

#[test]
fn a_topological_walk_never_puts_a_parent_before_its_child() {
    let f = interleaved();
    let repo = RepoHandle::open(f.path()).unwrap();

    let order = walked(&repo);

    let at = |what: &str| order.iter().position(|row| row == what).unwrap();
    assert!(at("commit 40") < at("commit 20"));
    assert!(at("commit 30") < at("commit 10"));
}

#[test]
fn a_topological_walk_of_a_linear_history_is_the_history() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    let order = walked(&repo);

    assert_eq!(order.len(), 3);
}
