// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use test_fixtures::{branched, diamond, octopus, two_roots};

fn parent_count(fixture: &test_fixtures::Fixture, rev: &str) -> usize {
    let line = fixture
        .git(&["rev-list", "--parents", "-n", "1", rev])
        .unwrap();
    line.split_whitespace().count() - 1
}

fn count(fixture: &test_fixtures::Fixture, args: &[&str]) -> usize {
    fixture.git(args).unwrap().trim().parse().unwrap()
}

#[test]
fn branched_leaves_the_side_branch_unmerged() {
    let f = branched().unwrap();
    let branches = f.git(&["branch", "--format=%(refname:short)"]).unwrap();
    assert!(
        branches.contains("dev"),
        "expected a dev branch, got: {branches}"
    );
    assert_eq!(count(&f, &["rev-list", "--merges", "--count", "main"]), 0);
    assert!(count(&f, &["rev-list", "--count", "main..dev"]) > 0);
}

#[test]
fn diamond_has_exactly_one_merge_commit() {
    let f = diamond().unwrap();
    assert_eq!(count(&f, &["rev-list", "--merges", "--count", "HEAD"]), 1);
}

#[test]
fn diamond_merge_joins_two_parents() {
    let f = diamond().unwrap();
    assert_eq!(parent_count(&f, "HEAD"), 2);
}

#[test]
fn diamond_parents_share_a_single_merge_base() {
    let f = diamond().unwrap();
    let base = f.git(&["merge-base", "HEAD^1", "HEAD^2"]).unwrap();
    assert!(
        !base.trim().is_empty(),
        "the two sides must share an ancestor"
    );
}

#[test]
fn octopus_merge_joins_three_parents() {
    let f = octopus().unwrap();
    assert_eq!(parent_count(&f, "HEAD"), 3);
}

#[test]
fn two_roots_has_two_parentless_commits() {
    let f = two_roots().unwrap();
    let roots = f.git(&["rev-list", "--max-parents=0", "--all"]).unwrap();
    assert_eq!(roots.trim().lines().count(), 2, "expected two root commits");
}

#[test]
fn two_roots_histories_never_meet() {
    let f = two_roots().unwrap();
    let out = f.git(&["merge-base", "main", "orphan"]);
    assert!(
        out.is_err(),
        "independent roots must not share a merge base"
    );
}
