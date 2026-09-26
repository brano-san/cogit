// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::graph_overlay::{GraphPaintRequest, PaintTip};
use app_state::{AppState, RepoId};
use git_engine::CommitQuery;
use graph_engine::PAINT_DIM;

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
        ..GraphPaintRequest::default()
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
fn ancestry_dims_the_other_branch() {
    let f = test_fixtures::branched().unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let generation = build(&state, repo);
    let order = oids(&state, repo, generation);
    let main = f.oid("main").unwrap();

    let request = GraphPaintRequest {
        ancestry_of: Some(f.oid("dev~1").unwrap()),
        ..GraphPaintRequest::default()
    };
    let overlay = state
        .graph_overlay(repo, generation, 0, 100, &request)
        .unwrap();

    for (oid, style) in order.iter().zip(&overlay.node_styles) {
        assert_eq!(style & PAINT_DIM != 0, *oid == main, "{oid}");
    }
}

#[test]
fn mergeable_dims_what_a_merge_of_the_chosen_commit_would_not_bring() {
    let f = test_fixtures::branched().unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let generation = build(&state, repo);
    let order = oids(&state, repo, generation);
    let dev = [f.oid("dev").unwrap(), f.oid("dev~1").unwrap()];

    let request = GraphPaintRequest {
        mergeable_of: Some(dev[0].clone()),
        ..GraphPaintRequest::default()
    };
    let overlay = state
        .graph_overlay(repo, generation, 0, 100, &request)
        .unwrap();

    for (oid, style) in order.iter().zip(&overlay.node_styles) {
        assert_eq!(style & PAINT_DIM != 0, !dev.contains(oid), "{oid}");
    }
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

/// Paint is kept beside the rows of every cached graph, so the byte budget counts it too
/// (R-300): the index alone copies every oid.
#[test]
fn the_cache_budget_counts_the_paint_kept_with_a_graph() {
    let f = test_fixtures::branched().unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let generation = build(&state, repo);
    // The texts read ahead count as well; they have to be in before the rows are measured.
    let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while state.graph_texts_read(repo) < 4 && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let rows = state.graph_footprint(repo);

    let request = GraphPaintRequest {
        tips: vec![PaintTip {
            oid: f.oid("dev").unwrap(),
            slot: 5,
        }],
        ..GraphPaintRequest::default()
    };
    state
        .graph_overlay(repo, generation, 0, 100, &request)
        .unwrap();

    assert!(state.graph_footprint(repo) > rows + 4 * 40);
}
