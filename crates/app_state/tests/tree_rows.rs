//! The Repositories panel asks for the whole tree on every refresh. Reading git for a row
//! nothing happened to is waste, and on a repository the size of `dtv_device` it is the
//! waste that made the panel stutter (problem 5).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::AppState;

fn opened() -> (AppState, app_state::RepoId, test_fixtures::Fixture) {
    let fixture = test_fixtures::linear(2).unwrap();
    let state = AppState::new();
    let repo = state.register(fixture.path().to_path_buf(), "fixture".into());
    (state, repo, fixture)
}

#[test]
fn a_quiet_repository_is_read_once_however_often_the_panel_asks() {
    let (state, _, _fixture) = opened();

    for _ in 0..5 {
        assert_eq!(state.overviews().len(), 1);
    }

    assert_eq!(state.rows_read(), 1, "four of those five were free");
}

#[test]
fn forgetting_a_row_makes_the_next_look_real() {
    let (state, repo, fixture) = opened();
    assert!(!state.overviews()[0].dirty);

    std::fs::write(fixture.path().join("new.txt"), "x").unwrap();
    state.forget_row(repo);

    assert!(
        state.overviews()[0].dirty,
        "the row was read again, not served from before"
    );
    assert_eq!(state.rows_read(), 2);
}

#[test]
fn forgetting_one_repository_leaves_the_others_cached() {
    let (state, repo, _fixture) = opened();
    let other = test_fixtures::linear(1).unwrap();
    state.register(other.path().to_path_buf(), "other".into());

    assert_eq!(state.overviews().len(), 2);
    assert_eq!(state.rows_read(), 2);

    state.forget_row(repo);
    let _ = state.overviews();

    assert_eq!(state.rows_read(), 3, "only the one that was forgotten");
}

#[test]
fn closing_a_repository_takes_its_row_with_it() {
    let (state, repo, _fixture) = opened();
    let _ = state.overviews();

    assert!(state.close_repository(repo));
    assert!(state.overviews().is_empty());
}
