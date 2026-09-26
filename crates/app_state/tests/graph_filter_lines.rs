// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Show Graph While Filtering (F-561): the matches of a filter joined by lines through the
//! commits it leaves out, instead of a flat list with arrows.

use app_state::{AppState, GraphChunk};
use git_engine::{CommitQuery, GraphView};
use graph_engine::{GraphRow, Span};
use test_fixtures::Fixture;

/// Five commits on one line, alternating between two files: `a.txt` is in 0, 2 and 4.
fn alternating() -> Fixture {
    let f = Fixture::init().unwrap();
    for index in 0..5 {
        let name = if index % 2 == 0 { "a.txt" } else { "b.txt" };
        f.commit_file(index, name, &format!("version {index}\n"))
            .unwrap();
    }
    f
}

fn rows(state: &AppState, repo: app_state::RepoId, lines: bool) -> Vec<GraphRow> {
    let query = CommitQuery {
        path: Some("a.txt".to_owned()),
        view: GraphView {
            filtered_graph: lines,
            ..GraphView::default()
        },
        ..CommitQuery::default()
    };
    let mut chunks: Vec<GraphChunk> = Vec::new();
    state
        .search_graph(repo, &query, 100, |chunk| {
            chunks.push(chunk);
            true
        })
        .unwrap();
    chunks.into_iter().flat_map(|chunk| chunk.rows).collect()
}

fn lines_into_node(row: &GraphRow) -> usize {
    row.segments
        .iter()
        .filter(|s| s.span == Span::Top && s.to == row.lane && !s.arrow)
        .count()
}

#[test]
fn a_filtered_list_is_flat_by_default() {
    let f = alternating();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let rows = rows(&state, repo, false);
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|row| lines_into_node(row) == 0));
    assert!(rows[0].segments.iter().any(|s| s.arrow), "{:?}", rows[0]);
}

#[test]
fn with_the_graph_each_match_joins_the_next_one_down() {
    let f = alternating();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let rows = rows(&state, repo, true);
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|row| row.segments.iter().all(|s| !s.arrow)));
    assert_eq!(lines_into_node(&rows[1]), 1, "{:?}", rows[1]);
    assert_eq!(lines_into_node(&rows[2]), 1, "{:?}", rows[2]);
    assert!(rows.iter().all(|row| row.primary && row.lane == 0));
}
