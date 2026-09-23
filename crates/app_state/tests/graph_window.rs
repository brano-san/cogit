// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, GraphProgress, RepoId};
use git_engine::CommitQuery;

fn build(state: &AppState, repo: RepoId, chunk_size: usize) -> (u32, Vec<GraphProgress>) {
    let generation = state.begin_graph();
    let mut progress = Vec::new();
    state
        .build_graph(repo, &CommitQuery::default(), generation, chunk_size, |p| {
            progress.push(p);
            true
        })
        .unwrap();
    (generation, progress)
}

fn open(f: &test_fixtures::Fixture) -> (AppState, RepoId) {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    (state, repo)
}

#[test]
fn a_window_holds_the_rows_asked_for() {
    let f = test_fixtures::linear(10).unwrap();
    let (state, repo) = open(&f);
    let (generation, _) = build(&state, repo, 4);

    let window = state.graph_window(repo, generation, 2, 3).unwrap();

    assert_eq!(window.start, 2);
    assert_eq!(
        window.rows.iter().map(|r| r.row).collect::<Vec<_>>(),
        [2, 3, 4]
    );
    assert_eq!(window.commits.len(), 3);
    assert_eq!(window.commits[0].summary, "commit 7");
    assert_eq!((window.total, window.complete), (10, true));
}

#[test]
fn a_window_past_the_end_is_cut_short() {
    let f = test_fixtures::linear(10).unwrap();
    let (state, repo) = open(&f);
    let (generation, _) = build(&state, repo, 4);

    let window = state.graph_window(repo, generation, 8, 10).unwrap();
    assert_eq!(window.rows.len(), 2);

    let beyond = state.graph_window(repo, generation, 50, 10).unwrap();
    assert!(beyond.rows.is_empty());
}

#[test]
fn a_window_of_a_replaced_graph_is_none() {
    let f = test_fixtures::linear(5).unwrap();
    let (state, repo) = open(&f);
    let (old, _) = build(&state, repo, 100);
    let (new, _) = build(&state, repo, 100);

    assert!(state.graph_window(repo, old, 0, 5).is_none());
    assert!(state.graph_window(repo, new, 0, 5).is_some());
}

#[test]
fn progress_counts_up_to_the_whole_history_and_ends_once() {
    let f = test_fixtures::linear(10).unwrap();
    let (state, repo) = open(&f);

    let (generation, progress) = build(&state, repo, 4);

    let totals: Vec<u32> = progress.iter().map(|p| p.total).collect();
    assert_eq!(totals, [4, 8, 10, 10]);
    assert_eq!(progress.iter().filter(|p| p.is_last).count(), 1);
    assert!(progress.last().unwrap().is_last);
    assert!(progress.iter().all(|p| p.generation == generation));
}

#[test]
fn an_empty_repository_reports_an_empty_finished_graph() {
    let f = test_fixtures::empty().unwrap();
    let (state, repo) = open(&f);

    let (generation, progress) = build(&state, repo, 100);

    assert_eq!(progress.len(), 1);
    assert_eq!((progress[0].total, progress[0].is_last), (0, true));
    let window = state.graph_window(repo, generation, 0, 10).unwrap();
    assert!(window.rows.is_empty() && window.complete);
}

#[test]
fn a_commit_is_found_by_its_oid() {
    let f = test_fixtures::linear(6).unwrap();
    let (state, repo) = open(&f);
    let (generation, _) = build(&state, repo, 2);
    let oid = state.graph_window(repo, generation, 3, 1).unwrap().commits[0]
        .oid
        .clone();

    assert_eq!(state.graph_row_of(repo, generation, &oid), Some(3));
    assert_eq!(state.graph_row_of(repo, generation, "0000000"), None);
}

#[test]
fn a_newer_graph_stops_the_walk_of_the_older_one() {
    let f = test_fixtures::linear(10).unwrap();
    let (state, repo) = open(&f);
    let generation = state.begin_graph();
    let mut seen = Vec::new();

    state
        .build_graph(repo, &CommitQuery::default(), generation, 2, |p| {
            seen.push(p.total);
            state.begin_graph();
            true
        })
        .unwrap();

    assert_eq!(
        seen,
        [2],
        "the walk ends at the first chunk after it was replaced"
    );
}
