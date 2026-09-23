// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::RepoHandle;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

#[test]
fn a_branch_that_forked_off_is_not_in_head() {
    let f = test_fixtures::branched().unwrap();
    assert!(!open(&f).is_merged_into_head("dev").unwrap());
}

#[test]
fn an_ancestor_of_head_is_already_in_it() {
    let f = test_fixtures::branched().unwrap();
    assert!(open(&f).is_merged_into_head("main~1").unwrap());
}

#[test]
fn head_itself_counts_as_merged() {
    let f = test_fixtures::branched().unwrap();
    let head = f.oid("HEAD").unwrap();
    assert!(open(&f).is_merged_into_head(&head).unwrap());
}

#[test]
fn a_branch_merged_earlier_is_in_head() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["merge", "--no-ff", "-m", "merge dev", "dev"])
        .unwrap();
    assert!(open(&f).is_merged_into_head("dev").unwrap());
}

#[test]
fn an_unknown_revision_is_an_error() {
    let f = test_fixtures::branched().unwrap();
    assert!(open(&f).is_merged_into_head("no-such-branch").is_err());
}

/// `origin` holds `alive`, `gone-merged` and `gone-unmerged`; the last two are then
/// deleted there and pruned here, as a teammate's cleanup after merging would leave them.
fn after_cleanup_on_the_remote() -> test_fixtures::Fixture {
    let f = test_fixtures::with_remote().unwrap();
    for name in ["alive", "gone-merged", "local-only"] {
        f.git(&["branch", name, "main~1"]).unwrap();
    }
    f.git(&["switch", "-c", "gone-unmerged"]).unwrap();
    f.commit_file(40, "unmerged.txt", "only here\n").unwrap();
    f.git(&["switch", "main"]).unwrap();
    for name in ["alive", "gone-merged", "gone-unmerged"] {
        f.git(&["push", "--set-upstream", "origin", name]).unwrap();
    }
    f.git(&["push", "origin", "--delete", "gone-merged", "gone-unmerged"])
        .unwrap();
    f.git(&["fetch", "--prune", "origin"]).unwrap();
    f
}

#[test]
fn a_merged_branch_whose_upstream_was_deleted_is_offered_for_deletion() {
    let f = after_cleanup_on_the_remote();
    assert_eq!(open(&f).merged_gone_branches().unwrap(), ["gone-merged"]);
}

#[test]
fn a_branch_whose_upstream_still_exists_is_kept() {
    let f = after_cleanup_on_the_remote();
    let names = open(&f).merged_gone_branches().unwrap();
    assert!(!names.contains(&"alive".to_owned()), "{names:?}");
}

#[test]
fn a_branch_never_pushed_is_kept() {
    let f = after_cleanup_on_the_remote();
    let names = open(&f).merged_gone_branches().unwrap();
    assert!(!names.contains(&"local-only".to_owned()), "{names:?}");
}

#[test]
fn a_branch_with_commits_head_lacks_is_kept_even_when_gone() {
    let f = after_cleanup_on_the_remote();
    let names = open(&f).merged_gone_branches().unwrap();
    assert!(!names.contains(&"gone-unmerged".to_owned()), "{names:?}");
}

#[test]
fn the_checked_out_branch_is_never_offered() {
    let f = after_cleanup_on_the_remote();
    f.git(&["switch", "gone-merged"]).unwrap();
    assert!(open(&f).merged_gone_branches().unwrap().is_empty());
}

#[test]
fn a_branch_checked_out_in_another_worktree_is_kept() {
    let f = after_cleanup_on_the_remote();
    let elsewhere = f.path().join("elsewhere");
    f.git(&[
        "worktree",
        "add",
        &elsewhere.to_string_lossy(),
        "gone-merged",
    ])
    .unwrap();
    assert!(open(&f).merged_gone_branches().unwrap().is_empty());
}
