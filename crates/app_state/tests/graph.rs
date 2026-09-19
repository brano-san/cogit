// Integration tests are test code by definition, but `allow-unwrap-in-tests` in
// clippy.toml only covers the bodies of `#[test]` functions — helpers beside them are
// still linted. Panicking is how a test reports failure, so allow it for the file.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, GraphChunk, RepoId};

fn stream(state: &AppState, repo: RepoId, chunk_size: usize) -> Vec<GraphChunk> {
    let mut chunks = Vec::new();
    state
        .stream_graph(repo, chunk_size, |chunk| {
            chunks.push(chunk);
            true
        })
        .unwrap();
    chunks
}

#[test]
fn every_commit_arrives_with_a_lane() {
    let f = test_fixtures::linear(5).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let chunks = stream(&state, repo, 100);
    let commits: usize = chunks.iter().map(|c| c.commits.len()).sum();
    let lanes: usize = chunks.iter().map(|c| c.lanes.len()).sum();
    assert_eq!(commits, 5);
    assert_eq!(lanes, commits, "each commit needs exactly one placement");
}

#[test]
fn the_stream_ends_with_a_chunk_marked_last() {
    let f = test_fixtures::linear(3).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let chunks = stream(&state, repo, 100);
    assert!(
        chunks.last().unwrap().is_last,
        "the UI needs to know when to stop waiting"
    );
    assert_eq!(chunks.iter().filter(|c| c.is_last).count(), 1);
}

#[test]
fn an_empty_repository_still_reports_completion() {
    let f = test_fixtures::empty().unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let chunks = stream(&state, repo, 100);
    assert_eq!(chunks.len(), 1);
    assert!(chunks[0].is_last);
    assert!(chunks[0].commits.is_empty());
}

#[test]
fn rows_keep_counting_across_chunk_boundaries() {
    let f = test_fixtures::linear(7).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let rows: Vec<u32> = stream(&state, repo, 3)
        .iter()
        .flat_map(|c| c.lanes.iter().map(|l| l.row))
        .collect();
    assert_eq!(rows, (0..7).collect::<Vec<u32>>());
}

#[test]
fn a_merge_keeps_its_two_lanes_within_one_chunk() {
    let f = test_fixtures::diamond().unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let widest = stream(&state, repo, 100)
        .iter()
        .map(|c| c.max_lane)
        .max()
        .unwrap();
    assert_eq!(widest, 1);
}

#[test]
fn refusing_a_chunk_stops_the_stream() {
    let f = test_fixtures::linear(40).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let mut seen = 0;
    let mut saw_last = false;
    state
        .stream_graph(repo, 10, |chunk| {
            seen += chunk.commits.len();
            saw_last |= chunk.is_last;
            false
        })
        .unwrap();

    assert_eq!(seen, 10);
    assert!(!saw_last, "a cancelled stream must not claim it finished");
}

#[test]
fn an_unknown_repository_is_reported_as_missing() {
    let state = AppState::new();
    let result = state.stream_graph(RepoId(999), 10, |_| true);
    assert!(result.is_err());
}
