// Integration tests are test code by definition, but `allow-unwrap-in-tests` in
// clippy.toml only covers the bodies of `#[test]` functions — helpers beside them are
// still linted. Panicking is how a test reports failure, so allow it for the file.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Structural checks for the generated repository shapes.
//!
//! Every shape is verified with the **system `git`**, not with our own helpers: the
//! point of a fixture is to match how Git really behaves, so asking Git is the only
//! answer worth trusting.

use test_fixtures::{branched, diamond, octopus, two_roots};

/// Number of parents of a revision, as Git reports them.
fn parent_count(fixture: &test_fixtures::Fixture, rev: &str) -> usize {
    let line = fixture
        .git(&["rev-list", "--parents", "-n", "1", rev])
        .unwrap();
    // `rev-list --parents` prints "<commit> <parent>..." — subtract the commit itself.
    line.split_whitespace().count() - 1
}

fn count(fixture: &test_fixtures::Fixture, args: &[&str]) -> usize {
    fixture.git(args).unwrap().trim().parse().unwrap()
}

#[test]
fn branched_leaves_the_side_branch_unmerged() {
    let f = branched().unwrap();
    // The branch exists...
    let branches = f.git(&["branch", "--format=%(refname:short)"]).unwrap();
    assert!(
        branches.contains("dev"),
        "expected a dev branch, got: {branches}"
    );
    // ...and was never merged, so main has no merge commits.
    assert_eq!(count(&f, &["rev-list", "--merges", "--count", "main"]), 0);
    // dev really diverged: it holds commits main does not.
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
    // What makes it a diamond rather than two unrelated lines.
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
    // No merge base means the histories are genuinely independent.
    let out = f.git(&["merge-base", "main", "orphan"]);
    assert!(
        out.is_err(),
        "independent roots must not share a merge base"
    );
}
