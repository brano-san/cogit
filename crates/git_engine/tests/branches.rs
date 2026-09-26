// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{CheckoutTarget, Head, RepoHandle};

fn open(fixture: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(fixture.path()).unwrap()
}

fn names(repo: &RepoHandle) -> Vec<String> {
    repo.branches()
        .unwrap()
        .into_iter()
        .map(|b| b.name)
        .collect()
}

#[test]
fn checking_out_a_branch_moves_head_to_it() {
    let f = test_fixtures::branched().unwrap();
    let repo = open(&f);

    repo.checkout(&CheckoutTarget::Branch {
        name: "dev".to_owned(),
    })
    .unwrap();

    match repo.head().unwrap() {
        Head::Branch { name, .. } => assert_eq!(name, "dev"),
        other => panic!("expected an attached HEAD, got {other:?}"),
    }
}

#[test]
fn checking_out_a_commit_detaches_head() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);
    let target = f.oid("HEAD~1").unwrap();

    repo.checkout(&CheckoutTarget::Commit {
        oid: target.clone(),
    })
    .unwrap();

    match repo.head().unwrap() {
        Head::Detached { oid } => assert_eq!(oid, target),
        other => panic!("INV-07: a detached HEAD is a normal state, got {other:?}"),
    }
}

fn refusal(result: Result<(), git_engine::GitError>) -> Box<git_engine::GitCommandError> {
    match result {
        Err(git_engine::GitError::Command(details)) => details,
        other => panic!("INV-05: expected git's own refusal, got {other:?}"),
    }
}

fn head_branch(repo: &RepoHandle) -> String {
    match repo.head().unwrap() {
        Head::Branch { name, .. } => name,
        other => panic!("expected an attached HEAD, got {other:?}"),
    }
}

// The headings are the ones `frontend/src/lib/checkout-refusal.ts` offers a stash for.
#[test]
fn checking_out_with_uncommitted_changes_reports_gits_refusal() {
    let f = test_fixtures::branched().unwrap();
    std::fs::write(f.path().join("dev-1.txt"), "would be overwritten\n").unwrap();
    let repo = open(&f);

    let details = refusal(repo.checkout(&CheckoutTarget::Branch {
        name: "dev".to_owned(),
    }));

    assert!(
        details
            .stderr
            .contains("The following untracked working tree files would be overwritten by"),
        "{details:?}"
    );
    assert!(details.stderr.contains("dev-1.txt"), "{details:?}");
    assert_eq!(head_branch(&repo), "main");
    assert_eq!(
        std::fs::read_to_string(f.path().join("dev-1.txt")).unwrap(),
        "would be overwritten\n"
    );
}

#[test]
fn checking_out_over_a_changed_tracked_file_reports_gits_refusal() {
    let f = test_fixtures::branched().unwrap();
    std::fs::write(f.path().join("main-1.txt"), "changed here\n").unwrap();
    let repo = open(&f);

    let details = refusal(repo.checkout(&CheckoutTarget::Branch {
        name: "dev".to_owned(),
    }));

    assert!(
        details
            .stderr
            .contains("Your local changes to the following files would be overwritten by"),
        "{details:?}"
    );
    assert!(details.stderr.contains("main-1.txt"), "{details:?}");
    assert_eq!(head_branch(&repo), "main");
    assert_eq!(
        std::fs::read_to_string(f.path().join("main-1.txt")).unwrap(),
        "changed here\n"
    );
}

#[test]
fn creating_a_branch_adds_it_without_switching() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);

    repo.create_branch("topic", None, false).unwrap();

    assert!(names(&repo).contains(&"topic".to_owned()));
    match repo.head().unwrap() {
        Head::Branch { name, .. } => assert_ne!(name, "topic"),
        other => panic!("expected to stay put, got {other:?}"),
    }
}

#[test]
fn creating_a_branch_can_switch_to_it_at_once() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);

    repo.create_branch("topic", None, true).unwrap();

    match repo.head().unwrap() {
        Head::Branch { name, .. } => assert_eq!(name, "topic"),
        other => panic!("expected to be on topic, got {other:?}"),
    }
}

#[test]
fn a_branch_can_start_from_an_older_commit() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);
    let start = f.oid("HEAD~2").unwrap();

    repo.create_branch("old", Some(&start), false).unwrap();

    let branch = repo
        .branches()
        .unwrap()
        .into_iter()
        .find(|b| b.name == "old")
        .unwrap();
    assert_eq!(branch.oid, start);
}

#[test]
fn a_duplicate_branch_name_is_refused_with_gits_words() {
    let f = test_fixtures::branched().unwrap();
    let repo = open(&f);

    let err = repo.create_branch("dev", None, false).unwrap_err();

    match err {
        git_engine::GitError::Command(details) => {
            assert!(details.stderr.contains("dev"), "{details:?}");
        }
        other => panic!("expected a command failure, got {other:?}"),
    }
}

#[test]
fn deleting_a_merged_branch_removes_it() {
    let f = test_fixtures::diamond().unwrap();
    let repo = open(&f);

    repo.delete_branch("dev", false).unwrap();

    assert!(!names(&repo).contains(&"dev".to_owned()));
}

#[test]
fn deleting_an_unmerged_branch_needs_force() {
    let f = test_fixtures::branched().unwrap();
    let repo = open(&f);

    assert!(
        repo.delete_branch("dev", false).is_err(),
        "losing commits must take an explicit force"
    );
    repo.delete_branch("dev", true).unwrap();
    assert!(!names(&repo).contains(&"dev".to_owned()));
}

#[test]
fn the_current_branch_cannot_be_deleted() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);
    let current = match repo.head().unwrap() {
        Head::Branch { name, .. } => name,
        other => panic!("expected an attached HEAD, got {other:?}"),
    };

    assert!(repo.delete_branch(&current, true).is_err());
}

#[test]
fn a_branch_name_with_a_slash_is_handled() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    repo.create_branch("feature/nested-name", None, false)
        .unwrap();

    assert!(names(&repo).contains(&"feature/nested-name".to_owned()));
}

#[test]
fn an_empty_branch_name_is_refused_before_git_is_started() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    assert!(repo.create_branch("  ", None, false).is_err());
    assert!(repo.delete_branch("", false).is_err());
}
