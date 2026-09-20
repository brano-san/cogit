// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{RepoHandle, RepoState};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

#[test]
fn cherry_picking_brings_one_commit_across() {
    let f = test_fixtures::branched().unwrap();
    let wanted = f.oid("dev").unwrap();
    let repo = open(&f);

    repo.cherry_pick(std::slice::from_ref(&wanted)).unwrap();

    let head = repo.commit_details("HEAD").unwrap();
    assert_eq!(head.summary, repo.commit_details(&wanted).unwrap().summary);
    assert_ne!(head.oid, wanted, "a cherry-pick makes a new commit");
}

#[test]
fn cherry_picking_several_commits_keeps_their_order() {
    let f = test_fixtures::branched().unwrap();
    let older = f.oid("dev~1").unwrap();
    let newer = f.oid("dev").unwrap();
    let repo = open(&f);

    repo.cherry_pick(&[older, newer]).unwrap();

    assert_eq!(repo.commit_details("HEAD").unwrap().summary, "commit 3");
    assert_eq!(repo.commit_details("HEAD~1").unwrap().summary, "commit 2");
}

#[test]
fn a_conflicting_cherry_pick_stops_in_that_state() {
    let f = test_fixtures::branched().unwrap();
    f.write_file("base.txt", "main version\n").unwrap();
    f.git(&["add", "--", "base.txt"]).unwrap();
    f.commit_staged(30, "change base on main").unwrap();
    f.git(&["switch", "dev"]).unwrap();
    f.write_file("base.txt", "dev version\n").unwrap();
    f.git(&["add", "--", "base.txt"]).unwrap();
    f.commit_staged(31, "change base on dev").unwrap();
    let conflicting = f.oid("HEAD").unwrap();
    f.git(&["switch", "main"]).unwrap();
    let repo = open(&f);

    assert!(repo.cherry_pick(&[conflicting]).is_err());
    assert_eq!(repo.state().unwrap(), RepoState::CherryPicking);
}

#[test]
fn reverting_undoes_a_commit_with_a_new_one() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);
    let target = f.oid("HEAD").unwrap();

    repo.revert(&[target]).unwrap();

    assert!(
        !f.path().join("file2.txt").exists(),
        "the file the commit added must be gone again"
    );
    assert_eq!(repo.commit_details("HEAD").unwrap().parents.len(), 1);
}

#[test]
fn reverting_keeps_the_original_commit_in_history() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);
    let target = f.oid("HEAD").unwrap();

    repo.revert(std::slice::from_ref(&target)).unwrap();

    assert!(
        repo.commit_details(&target).is_ok(),
        "history is not rewritten"
    );
}

#[test]
fn reverting_a_merge_without_a_mainline_is_refused_by_git() {
    let f = test_fixtures::diamond().unwrap();
    let repo = open(&f);
    let merge = f.oid("HEAD").unwrap();

    let err = repo.revert(&[merge]).unwrap_err();

    match err {
        git_engine::GitError::Command(details) => {
            assert!(
                details.stderr.contains("is a merge"),
                "the user needs to know which side to revert against, got {:?}",
                details.stderr
            );
        }
        other => panic!("expected Git to explain itself, got {other:?}"),
    }
}

#[test]
fn an_empty_list_is_refused_before_git_is_started() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);

    assert!(repo.cherry_pick(&[]).is_err());
    assert!(repo.revert(&[]).is_err());
}

#[test]
fn an_unknown_commit_reports_gits_words() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);

    assert!(repo.cherry_pick(&["0".repeat(40)]).is_err());
}

#[test]
fn neither_operation_opens_an_editor() {
    let f = test_fixtures::branched().unwrap();
    let wanted = f.oid("dev").unwrap();
    let repo = open(&f);

    // Both would hang forever if the editor were reachable (doc/12-risks.md, R-26).
    repo.cherry_pick(&[wanted]).unwrap();
    repo.revert(&[f.oid("HEAD").unwrap()]).unwrap();

    assert_eq!(repo.state().unwrap(), RepoState::Clean);
}
