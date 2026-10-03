#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Deleting every ref under a folder of Branches in one queued step.

use app_state::{AppState, RefDeletion, RefDeletionKind, RepoId, split_deletable};

fn open(f: &test_fixtures::Fixture) -> (AppState, RepoId) {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    (state, repo)
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| (*name).to_owned()).collect()
}

fn request(kind: RefDeletionKind, list: &[&str], force: bool) -> RefDeletion {
    RefDeletion {
        kind,
        remote: None,
        names: names(list),
        force,
    }
}

#[test]
fn checked_out_branches_are_split_off_with_their_reason() {
    let held = vec![
        ("main".to_owned(), None),
        ("wip".to_owned(), Some("linked".to_owned())),
    ];

    let (ok, skipped) = split_deletable(&names(&["main", "wip", "spare"]), &held);

    assert_eq!(ok, names(&["spare"]));
    let why: Vec<(&str, &str)> = skipped
        .iter()
        .map(|s| (s.name.as_str(), s.reason.as_str()))
        .collect();
    assert_eq!(
        why,
        [
            ("main", "it is the current branch"),
            ("wip", "it is checked out in the worktree linked"),
        ]
    );
}

#[test]
fn a_folder_of_branches_goes_but_the_current_one_stays() {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["branch", "fix/a"]).unwrap();
    f.git(&["branch", "fix/b"]).unwrap();
    f.git(&["checkout", "-q", "-b", "fix/current"]).unwrap();
    let (state, repo) = open(&f);

    let report = state
        .delete_refs(
            repo,
            &request(
                RefDeletionKind::Branch,
                &["fix/a", "fix/b", "fix/current"],
                false,
            ),
            &git_engine::NetworkStop::default(),
        )
        .unwrap();

    assert_eq!(report.deleted, names(&["fix/a", "fix/b"]));
    assert_eq!(report.skipped.len(), 1);
    assert_eq!(report.skipped[0].name, "fix/current");
    assert!(f.git(&["rev-parse", "--verify", "fix/current"]).is_ok());
    assert!(f.git(&["rev-parse", "--verify", "fix/a"]).is_err());
}

#[test]
fn one_failure_does_not_stop_the_rest_and_keeps_gits_words() {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["branch", "fix/a"]).unwrap();
    f.git(&["branch", "fix/b"]).unwrap();
    let (state, repo) = open(&f);

    let report = state
        .delete_refs(
            repo,
            &request(
                RefDeletionKind::Branch,
                &["fix/a", "fix/missing", "fix/b"],
                false,
            ),
            &git_engine::NetworkStop::default(),
        )
        .unwrap();

    assert_eq!(report.deleted, names(&["fix/a", "fix/b"]));
    assert_eq!(report.failed.len(), 1);
    assert_eq!(report.failed[0].name, "fix/missing");
    let git_engine::GitError::Command(err) = &report.failed[0].error else {
        panic!("{:?}", report.failed[0].error);
    };
    assert!(err.stderr.contains("fix/missing"), "{}", err.stderr);
}

#[test]
fn unmerged_branches_are_listed_and_go_with_force() {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["checkout", "-q", "-b", "fix/wip"]).unwrap();
    f.git(&["commit", "-q", "--allow-empty", "-m", "own work"])
        .unwrap();
    f.git(&["checkout", "-q", "-"]).unwrap();
    let (state, repo) = open(&f);

    let first = state
        .delete_refs(
            repo,
            &request(RefDeletionKind::Branch, &["fix/wip"], false),
            &git_engine::NetworkStop::default(),
        )
        .unwrap();
    assert_eq!(first.not_fully_merged, names(&["fix/wip"]));
    assert!(first.deleted.is_empty());

    let second = state
        .delete_refs(
            repo,
            &request(RefDeletionKind::Branch, &["fix/wip"], true),
            &git_engine::NetworkStop::default(),
        )
        .unwrap();
    assert_eq!(second.deleted, names(&["fix/wip"]));
}

#[test]
fn a_folder_of_tags_goes() {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["tag", "v/1"]).unwrap();
    f.git(&["tag", "v/2"]).unwrap();
    let (state, repo) = open(&f);

    let report = state
        .delete_refs(
            repo,
            &request(RefDeletionKind::Tag, &["v/1", "v/2"], false),
            &git_engine::NetworkStop::default(),
        )
        .unwrap();

    assert_eq!(report.deleted, names(&["v/1", "v/2"]));
    assert_eq!(f.git(&["tag"]).unwrap().trim(), "");
}

// A folder of remote branches is one `push --delete` after another; Cancel must end the
// run, not fail every remaining branch on its own (GR-01).
#[test]
fn cancelling_a_remote_deletion_ends_the_batch() {
    let f = test_fixtures::with_remote().unwrap();
    let (state, repo) = open(&f);
    let stop = git_engine::NetworkStop::default();
    stop.stop();
    let mut cancelled = request(RefDeletionKind::RemoteBranch, &["a", "b", "c"], false);
    cancelled.remote = Some("origin".to_owned());

    let report = state.delete_refs(repo, &cancelled, &stop).unwrap();

    assert_eq!(report.failed.len(), 1, "{report:?}");
    assert!(matches!(
        report.failed[0].error,
        git_engine::GitError::Cancelled(_)
    ));
    assert!(report.deleted.is_empty());
}
