// Integration tests are test code by definition, but `allow-unwrap-in-tests` in
// clippy.toml only covers the bodies of `#[test]` functions — helpers beside them are
// still linted. Panicking is how a test reports failure, so allow it for the file.
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
    let f = test_fixtures::linear(50).unwrap();
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
