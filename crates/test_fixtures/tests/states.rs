// Integration tests are test code by definition, but `allow-unwrap-in-tests` in
// clippy.toml only covers the bodies of `#[test]` functions — helpers beside them are
// still linted. Panicking is how a test reports failure, so allow it for the file.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Repository states that Cogit must survive without panicking (INV-07).
//!
//! Every state here needs a banner in the UI, so the fixture has to reproduce the exact
//! marker files Git leaves behind — see `doc/03-git-semantics.md` section 4.

use test_fixtures::{bare, conflicted, detached_head, empty};

#[test]
fn empty_repository_has_no_commits() {
    let f = empty().unwrap();
    let count = f.git(&["rev-list", "--count", "--all"]).unwrap();
    assert_eq!(count.trim(), "0");
}

#[test]
fn empty_repository_has_an_unborn_head() {
    // HEAD points at a branch that does not exist yet. Resolving it must fail, and
    // that failure is what Cogit has to handle gracefully rather than crash on.
    let f = empty().unwrap();
    assert!(
        f.oid("HEAD").is_err(),
        "HEAD must not resolve in an empty repository"
    );
}

#[test]
fn bare_repository_has_no_working_tree() {
    let f = bare().unwrap();
    let out = f.git(&["rev-parse", "--is-bare-repository"]).unwrap();
    assert_eq!(out.trim(), "true");
}

#[test]
fn bare_repository_still_carries_its_history() {
    let f = bare().unwrap();
    let count: usize = f
        .git(&["rev-list", "--count", "HEAD"])
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(count > 0, "a bare clone must keep the commits");
}

#[test]
fn bare_fixture_reports_itself_as_the_git_directory() {
    // There is no `.git` subdirectory in a bare repository; the root *is* the git dir.
    let f = bare().unwrap();
    assert_eq!(f.git_dir(), f.path());
}

#[test]
fn detached_head_is_not_on_a_branch() {
    let f = detached_head().unwrap();
    let branch = f.git(&["branch", "--show-current"]).unwrap();
    assert!(
        branch.trim().is_empty(),
        "expected no current branch, got {branch:?}"
    );
}

#[test]
fn detached_head_still_resolves_to_a_commit() {
    let f = detached_head().unwrap();
    assert_eq!(f.oid("HEAD").unwrap().len(), 40);
}

#[test]
fn conflicted_leaves_a_merge_in_progress() {
    let f = conflicted().unwrap();
    assert!(
        f.git_dir().join("MERGE_HEAD").exists(),
        "MERGE_HEAD marks an unfinished merge"
    );
}

#[test]
fn conflicted_has_unmerged_paths() {
    let f = conflicted().unwrap();
    let unmerged = f.git(&["diff", "--name-only", "--diff-filter=U"]).unwrap();
    assert!(
        !unmerged.trim().is_empty(),
        "expected at least one conflicted file"
    );
}

#[test]
fn conflicted_index_holds_all_three_stages() {
    // Base, ours and theirs live in the index as stages 1, 2 and 3. The three-way
    // merge viewer in M7 reads exactly these.
    let f = conflicted().unwrap();
    let stages = f.git(&["ls-files", "-u"]).unwrap();
    for stage in ["1", "2", "3"] {
        assert!(
            stages
                .lines()
                .any(|l| l.split_whitespace().nth(2) == Some(stage)),
            "stage {stage} missing from:\n{stages}"
        );
    }
}
