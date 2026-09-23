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
