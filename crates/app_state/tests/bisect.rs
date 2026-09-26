// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, RepoId};
use git_engine::{BisectMark, RepoState};

fn open(f: &test_fixtures::Fixture) -> (AppState, RepoId) {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    state.show_repository(Some(repo));
    (state, repo)
}

/// A mark only checks out a commit; Reset forgets the marks, which Undo cannot bring back.
#[test]
fn only_the_reset_of_a_bisect_is_in_the_journal() {
    let f = test_fixtures::linear(8).unwrap();
    let (state, repo) = open(&f);

    state
        .bisect_start(repo, "HEAD", Some(&f.oid("main~7").unwrap()))
        .unwrap();
    state.bisect_mark(repo, BisectMark::Good, None).unwrap();
    assert!(state.safety_log().is_empty(), "{:?}", state.safety_log());

    state.bisect_reset(repo).unwrap();

    let log = state.safety_log();
    assert_eq!(log.len(), 1, "{log:?}");
    assert_eq!(log[0].description, "Reset the bisect");
    assert!(!log[0].undoable);
    assert_eq!(state.repo_refs(repo).unwrap().state, RepoState::Clean);
}

#[test]
fn the_refs_after_a_step_carry_the_bisect() {
    let f = test_fixtures::linear(8).unwrap();
    let (state, repo) = open(&f);

    state.bisect_start(repo, "HEAD", None).unwrap();

    match state.repo_refs(repo).unwrap().state {
        RepoState::Bisecting { bisect } => {
            assert_eq!(bisect.bad, Some(f.oid("main").unwrap()));
        }
        other => panic!("expected a bisect, got {other:?}"),
    }
}
