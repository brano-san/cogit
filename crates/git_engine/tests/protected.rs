#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Which commits must not be rewritten. Rewriting one that sits on a branch the team
//! shares costs everybody a divergence, so surgery is refused rather than warned about
//! (doc/modules/M12-commit-surgery.md).

use git_engine::RepoHandle;

/// `with_remote()` keeps two unpushed commits on top, so this is the last pushed one.
const PUSHED: &str = "HEAD~2";

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

#[test]
fn a_commit_on_the_main_branch_of_a_remote_is_protected() {
    let f = test_fixtures::with_remote().unwrap();
    let refs = open(&f).protecting_refs(PUSHED).unwrap();
    assert!(
        refs.iter().any(|name| name.contains("origin/main")),
        "{refs:?}"
    );
}

#[test]
fn a_commit_nobody_has_pushed_is_not_protected() {
    let f = test_fixtures::linear(2).unwrap();
    assert!(open(&f).protecting_refs("HEAD").unwrap().is_empty());
}

#[test]
fn a_commit_only_on_a_feature_branch_of_a_remote_is_not_protected() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["switch", "-c", "feature/work"]).unwrap();
    std::fs::write(f.path().join("new.txt"), "new\n").unwrap();
    f.git(&["add", "--", "new.txt"]).unwrap();
    f.git(&["commit", "-m", "on the feature branch"]).unwrap();
    f.git(&["push", "origin", "feature/work"]).unwrap();

    assert!(open(&f).protecting_refs("HEAD").unwrap().is_empty());
}

#[test]
fn a_release_branch_is_protected_by_the_default_pattern() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["switch", "-c", "release/1.0"]).unwrap();
    f.git(&["push", "origin", "release/1.0"]).unwrap();

    let refs = open(&f).protecting_refs("HEAD").unwrap();
    assert!(
        refs.iter().any(|name| name.contains("release/1.0")),
        "{refs:?}"
    );
}

#[test]
fn the_configured_list_replaces_the_default_one() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["config", "cogit.protectedBranches", "nothing-like-main"])
        .unwrap();

    assert!(open(&f).protecting_refs(PUSHED).unwrap().is_empty());
}

#[test]
fn an_empty_configured_list_protects_nothing() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["config", "cogit.protectedBranches", ""]).unwrap();

    assert!(open(&f).protecting_refs(PUSHED).unwrap().is_empty());
}

#[test]
fn a_configured_pattern_is_honoured() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["config", "cogit.protectedBranches", "ma*"])
        .unwrap();

    assert!(!open(&f).protecting_refs(PUSHED).unwrap().is_empty());
}

#[test]
fn the_answer_names_the_refs_so_the_reason_can_be_shown() {
    let f = test_fixtures::with_remote().unwrap();
    let refs = open(&f).protecting_refs(PUSHED).unwrap();
    assert!(refs.iter().all(|name| !name.is_empty()), "{refs:?}");
}

#[test]
fn a_revision_that_does_not_exist_is_an_error() {
    let f = test_fixtures::linear(1).unwrap();
    assert!(open(&f).protecting_refs("no-such-rev").is_err());
}
