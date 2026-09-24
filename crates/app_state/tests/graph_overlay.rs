// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::graph_overlay::{GraphPaintRequest, PaintTip};
use app_state::{AppState, RepoId};
use git_engine::CommitQuery;

fn build(state: &AppState, repo: RepoId) -> u32 {
    let generation = state.begin_graph();
    state
        .build_graph(repo, &CommitQuery::default(), generation, 2, |_| true)
        .unwrap();
    generation
}

fn oids(state: &AppState, repo: RepoId, generation: u32) -> Vec<String> {
    let window = state.graph_window(repo, generation, 0, 100).unwrap();
    window.commits.into_iter().map(|c| c.oid).collect()
}

#[test]
fn a_ticked_branch_comes_back_in_its_slot_and_the_main_line_does_not() {
    let f = test_fixtures::branched().unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let generation = build(&state, repo);
    let order = oids(&state, repo, generation);
    let dev = [f.oid("dev").unwrap(), f.oid("dev~1").unwrap()];

    let request = GraphPaintRequest {
        tips: vec![PaintTip {
            oid: dev[0].clone(),
            slot: 5,
        }],
    };
    let overlay = state
        .graph_overlay(repo, generation, 0, 100, &request)
        .unwrap();

    assert_eq!(overlay.total, 4);
    for (oid, style) in order.iter().zip(&overlay.node_styles) {
        let expected = if dev.contains(oid) { 6 } else { 0 };
        assert_eq!(*style, expected, "{oid}");
    }
}

#[test]
fn a_window_of_paint_lines_up_with_the_window_of_rows() {
    let f = test_fixtures::branched().unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let generation = build(&state, repo);

    let rows = state.graph_window(repo, generation, 1, 2).unwrap().rows;
    let overlay = state
        .graph_overlay(repo, generation, 1, 2, &GraphPaintRequest::default())
        .unwrap();

    assert_eq!(overlay.start, 1);
    assert_eq!(overlay.node_lanes.len(), 2);
    let segments: usize = rows.iter().map(|row| row.segments.len()).sum();
    assert_eq!(overlay.segment_lanes.len(), segments);
    assert_eq!(overlay.segment_styles.len(), segments);
    let counts: Vec<u32> = overlay
        .segment_first
        .windows(2)
        .map(|w| w[1] - w[0])
        .collect();
    let expected: Vec<u32> = rows
        .iter()
        .map(|row| u32::try_from(row.segments.len()).unwrap())
        .collect();
    assert_eq!(counts, expected);
}

#[test]
fn a_replaced_graph_has_no_paint() {
    let f = test_fixtures::linear(3).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let old = build(&state, repo);
    build(&state, repo);

    assert!(
        state
            .graph_overlay(repo, old, 0, 10, &GraphPaintRequest::default())
            .is_none()
    );
}
