// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{MergeOptions, RepoHandle, RepoState};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn plain(source: &str) -> MergeOptions {
    MergeOptions {
        source: source.to_owned(),
        no_fast_forward: false,
        squash: false,
        message: None,
    }
}

/// `branched()` diverges rather than staying ahead, so a fast-forward needs its own shape.
fn ahead_of_main() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["switch", "-c", "ahead"]).unwrap();
    f.commit_file(
        30,
        "ahead.txt",
        "one step ahead
",
    )
    .unwrap();
    f.git(&["switch", "main"]).unwrap();
    f
}

#[test]
fn merging_a_branch_that_is_ahead_fast_forwards() {
    let f = ahead_of_main();
    let repo = open(&f);

    repo.merge(&plain("ahead")).unwrap();

    assert_eq!(f.oid("HEAD").unwrap(), f.oid("ahead").unwrap());
    assert_eq!(repo.commit_details("HEAD").unwrap().parents.len(), 1);
    assert_eq!(repo.state().unwrap(), RepoState::Clean);
}

#[test]
fn no_fast_forward_always_records_a_merge_commit() {
    let f = ahead_of_main();
    let repo = open(&f);

    repo.merge(&MergeOptions {
        source: "ahead".to_owned(),
        no_fast_forward: true,
        squash: false,
        message: None,
    })
    .unwrap();

    assert_eq!(repo.commit_details("HEAD").unwrap().parents.len(), 2);
}

#[test]
fn merging_divergent_branches_creates_a_merge_commit() {
    let f = test_fixtures::branched().unwrap();
    f.commit_file(30, "main-2.txt", "main two\n").unwrap();
    let repo = open(&f);

    repo.merge(&plain("dev")).unwrap();

    let details = repo.commit_details("HEAD").unwrap();
    assert_eq!(details.parents.len(), 2);
    assert_eq!(repo.state().unwrap(), RepoState::Clean);
}

#[test]
fn a_conflicting_merge_leaves_the_repository_in_the_merging_state() {
    let f = test_fixtures::branched().unwrap();
    f.write_file("base.txt", "main version\n").unwrap();
    f.git(&["add", "--", "base.txt"]).unwrap();
    f.commit_staged(30, "change base on main").unwrap();
    f.git(&["switch", "dev"]).unwrap();
    f.write_file("base.txt", "dev version\n").unwrap();
    f.git(&["add", "--", "base.txt"]).unwrap();
    f.commit_staged(31, "change base on dev").unwrap();
    f.git(&["switch", "main"]).unwrap();
    let repo = open(&f);

    let result = repo.merge(&plain("dev"));

    assert!(result.is_err(), "a conflict is a failure the user must see");
    assert_eq!(repo.state().unwrap(), RepoState::Merging);
}

#[test]
fn a_squash_merge_leaves_the_changes_staged_without_committing() {
    let f = test_fixtures::branched().unwrap();
    f.commit_file(30, "main-2.txt", "main two\n").unwrap();
    let repo = open(&f);
    let before = f.oid("HEAD").unwrap();

    repo.merge(&MergeOptions {
        source: "dev".to_owned(),
        no_fast_forward: false,
        squash: true,
        message: None,
    })
    .unwrap();

    assert_eq!(f.oid("HEAD").unwrap(), before, "squash does not commit");
    assert!(!repo.worktree_files().unwrap().staged.is_empty());
}

#[test]
fn a_custom_message_is_used_for_the_merge_commit() {
    let f = test_fixtures::branched().unwrap();
    f.commit_file(30, "main-2.txt", "main two\n").unwrap();
    let repo = open(&f);

    repo.merge(&MergeOptions {
        source: "dev".to_owned(),
        no_fast_forward: false,
        squash: false,
        message: Some("bring dev in".to_owned()),
    })
    .unwrap();

    assert_eq!(repo.commit_details("HEAD").unwrap().summary, "bring dev in");
}

#[test]
fn merging_something_that_does_not_exist_reports_gits_words() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);

    let err = repo.merge(&plain("no-such-branch")).unwrap_err();

    match err {
        git_engine::GitError::Command(details) => {
            assert!(details.stderr.contains("no-such-branch"), "{details:?}");
        }
        other => panic!("expected a command failure, got {other:?}"),
    }
}

#[test]
fn an_empty_source_is_refused_before_git_is_started() {
    let f = test_fixtures::linear(2).unwrap();
    assert!(open(&f).merge(&plain("  ")).is_err());
}

#[test]
fn a_merge_never_opens_an_editor() {
    let f = test_fixtures::branched().unwrap();
    f.commit_file(30, "main-2.txt", "main two\n").unwrap();
    let repo = open(&f);

    // Would hang forever if the editor were reachable (doc/12-risks.md, R-26).
    repo.merge(&plain("dev")).unwrap();

    assert_eq!(repo.state().unwrap(), RepoState::Clean);
}

#[test]
fn a_squash_merge_seeds_the_commit_message_from_squash_msg() {
    let f = test_fixtures::branched().unwrap();
    f.commit_file(30, "main-2.txt", "main two\n").unwrap();
    let repo = open(&f);

    repo.merge(&MergeOptions {
        squash: true,
        ..plain("dev")
    })
    .unwrap();

    let seed = repo.message_seed().unwrap().unwrap();
    assert!(
        seed.starts_with("Squashed commit of the following:"),
        "{seed}"
    );
    assert!(seed.contains("commit 3"), "{seed}");
}

#[test]
fn the_squash_message_is_gone_once_the_squash_is_committed() {
    let f = test_fixtures::branched().unwrap();
    f.commit_file(30, "main-2.txt", "main two\n").unwrap();
    let repo = open(&f);
    repo.merge(&MergeOptions {
        squash: true,
        ..plain("dev")
    })
    .unwrap();

    repo.commit(&git_engine::CommitRequest {
        message: "squash dev".to_owned(),
        amend: false,
        no_verify: false,
        signoff: false,
        only: Vec::new(),
    })
    .unwrap();

    assert_eq!(repo.message_seed().unwrap(), None);
    assert_eq!(repo.commit_details("HEAD").unwrap().parents.len(), 1);
}

#[test]
fn a_stopped_merge_seeds_its_message_and_the_conflict_hints_stay_out() {
    let f = test_fixtures::conflicted().unwrap();
    let repo = open(&f);
    let seed = repo.message_seed().unwrap().unwrap();
    assert!(seed.starts_with("Merge branch"), "{seed}");
    assert!(seed.contains("# Conflicts:"), "{seed}");
    f.git(&["add", "-A"]).unwrap();

    repo.commit(&git_engine::CommitRequest {
        message: seed,
        amend: false,
        no_verify: false,
        signoff: false,
        only: Vec::new(),
    })
    .unwrap();

    let body = f.git(&["log", "-1", "--format=%B"]).unwrap();
    assert!(!body.contains('#'), "{body}");
}

#[test]
fn a_conflicted_squash_merge_seeds_the_squash_message_before_the_hints() {
    let f = test_fixtures::branched().unwrap();
    f.commit_file(30, "both.txt", "main side\n").unwrap();
    f.git(&["checkout", "--quiet", "dev"]).unwrap();
    f.commit_file(31, "both.txt", "dev side\n").unwrap();
    f.git(&["checkout", "--quiet", "main"]).unwrap();
    let _ = f.git(&["merge", "--squash", "dev"]);
    let repo = open(&f);

    let seed = repo.message_seed().unwrap().unwrap();

    assert!(
        seed.starts_with("Squashed commit of the following:"),
        "{seed}"
    );
    assert!(seed.contains("# Conflicts:"), "{seed}");
}

#[test]
fn fix_up_waits_for_a_stopped_merge() {
    let f = test_fixtures::conflicted().unwrap();
    f.git(&["add", "-A"]).unwrap();
    let repo = open(&f);

    let err = repo.commit_fixup("HEAD~1").unwrap_err();

    assert!(err.to_string().contains("in progress"), "{err}");
    assert_eq!(repo.state().unwrap(), RepoState::Merging);
}
