#![allow(clippy::unwrap_used, clippy::expect_used)]

//! What the four-panel merge view is given: the three sides already merged into regions,
//! so the UI counts conflicts rather than parsing markers out of a file (M7, doc/08 §8).

use app_state::AppState;

fn opened(f: &test_fixtures::Fixture) -> (AppState, app_state::RepoId) {
    let state = AppState::new();
    let summary = state.open_repository(f.path()).unwrap();
    (state, summary.repo)
}

#[test]
fn a_conflicted_file_comes_back_as_regions() {
    let f = test_fixtures::conflicted().unwrap();
    let (state, repo) = opened(&f);

    let regions = state.merge_preview(repo, "conflict.txt").unwrap();

    assert!(!regions.is_empty());
    assert!(regions.iter().any(diff_engine::Region::is_conflict));
}

#[test]
fn a_file_that_is_not_conflicted_is_an_error_rather_than_an_empty_merge() {
    let f = test_fixtures::conflicted().unwrap();
    let (state, repo) = opened(&f);
    assert!(state.merge_preview(repo, "no-such-file.txt").is_err());
}

#[test]
fn an_unknown_repo_id_is_an_error() {
    let state = AppState::new();
    assert!(
        state
            .merge_preview(app_state::RepoId(404), "a.txt")
            .is_err()
    );
}

#[test]
fn resolving_with_the_merged_text_clears_the_conflict() {
    let f = test_fixtures::conflicted().unwrap();
    let (state, repo) = opened(&f);

    let regions = state.merge_preview(repo, "conflict.txt").unwrap();
    let text: String = regions
        .iter()
        .flat_map(|region| match region {
            diff_engine::Region::Clean { lines, .. } => lines.clone(),
            diff_engine::Region::Conflict { ours, .. } => ours.clone(),
        })
        .collect::<Vec<_>>()
        .join("\n");

    state
        .resolve_conflict_text(repo, "conflict.txt", &format!("{text}\n"))
        .unwrap();

    assert!(state.conflicted_paths(repo).unwrap().is_empty());
}
