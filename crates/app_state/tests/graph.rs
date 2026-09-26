// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, GraphProgress, RepoId};

/// What the product shows: `build_graph`, then one window over every row (R-193).
struct Laid {
    commits: Vec<git_engine::CommitRow>,
    rows: Vec<graph_engine::GraphRow>,
}

fn laid_out(
    state: &AppState,
    repo: RepoId,
    query: &git_engine::CommitQuery,
    chunk_size: usize,
) -> (Vec<Laid>, Vec<GraphProgress>) {
    let generation = state.begin_graph();
    let mut progress = Vec::new();
    state
        .build_graph(repo, query, generation, chunk_size, |p| {
            progress.push(p);
            true
        })
        .unwrap();
    let window = state.graph_window(repo, generation, 0, u32::MAX).unwrap();
    let laid = Laid {
        commits: window.commits,
        rows: window.rows,
    };
    (vec![laid], progress)
}

fn stream(state: &AppState, repo: RepoId, chunk_size: usize) -> Vec<Laid> {
    laid_out(state, repo, &git_engine::CommitQuery::default(), chunk_size).0
}

#[test]
fn every_commit_arrives_with_a_lane() {
    let f = test_fixtures::linear(5).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let chunks = stream(&state, repo, 100);
    let commits: usize = chunks.iter().map(|c| c.commits.len()).sum();
    let rows: usize = chunks.iter().map(|c| c.rows.len()).sum();
    assert_eq!(commits, 5);
    assert_eq!(rows, commits, "each commit needs exactly one placement");
}

#[test]
fn the_stream_ends_with_a_chunk_marked_last() {
    let f = test_fixtures::linear(3).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let (_, progress) = laid_out(&state, repo, &git_engine::CommitQuery::default(), 1);
    assert!(
        progress.last().unwrap().is_last,
        "the UI needs to know when to stop waiting"
    );
    assert_eq!(progress.iter().filter(|p| p.is_last).count(), 1);
}

#[test]
fn an_empty_repository_still_reports_completion() {
    let f = test_fixtures::empty().unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let (laid, progress) = laid_out(&state, repo, &git_engine::CommitQuery::default(), 100);
    assert_eq!(progress.len(), 1);
    assert!(progress[0].is_last);
    assert!(laid[0].commits.is_empty());
}

#[test]
fn rows_keep_counting_across_chunk_boundaries() {
    let f = test_fixtures::linear(7).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let rows: Vec<u32> = stream(&state, repo, 3)
        .iter()
        .flat_map(|c| c.rows.iter().map(|l| l.row))
        .collect();
    assert_eq!(rows, (0..7).collect::<Vec<u32>>());
}

#[test]
fn a_diamond_is_two_columns_wide_while_it_is_open() {
    let f = test_fixtures::diamond().unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let widest = stream(&state, repo, 100)
        .iter()
        .flat_map(|c| c.rows.iter().map(|row| row.width))
        .max()
        .unwrap();
    assert_eq!(widest, 2);
}

/// Column 0 follows HEAD's first parents; with HEAD unticked it follows nothing, rather
/// than sitting empty for a line that is not drawn (doc/12-risks.md, R-161).
#[test]
fn the_main_column_follows_head_when_head_is_ticked_and_nothing_when_it_is_not() {
    let f = test_fixtures::linear(3).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let with_head = git_engine::CommitQuery {
        visible_refs: Some(vec!["HEAD".to_owned()]),
        ..git_engine::CommitQuery::default()
    };
    let rows: Vec<_> = search(&state, repo, &with_head)
        .into_iter()
        .flat_map(|c| c.rows)
        .collect();
    assert!(rows.iter().all(|row| row.primary && row.lane == 0));

    let without = git_engine::CommitQuery {
        visible_refs: Some(vec![
            "refs/heads/other".to_owned(),
            "refs/heads/main".to_owned(),
        ]),
        ..git_engine::CommitQuery::default()
    };
    let rows: Vec<_> = search(&state, repo, &without)
        .into_iter()
        .flat_map(|c| c.rows)
        .collect();
    assert!(
        rows.iter().all(|row| row.lane == 0),
        "main is ticked, so main takes column 0"
    );
}

#[test]
fn refusing_a_chunk_stops_the_stream() {
    // Two more than the chunk, so the first chunk cannot also be the last one.
    let f = test_fixtures::linear(12).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let generation = state.begin_graph();
    let mut seen = 0;
    let mut saw_last = false;
    state
        .build_graph(
            repo,
            &git_engine::CommitQuery::default(),
            generation,
            10,
            |progress| {
                seen = progress.total;
                saw_last |= progress.is_last;
                false
            },
        )
        .unwrap();

    assert_eq!(seen, 10);
    assert!(!saw_last, "a cancelled stream must not claim it finished");
    let window = state.graph_window(repo, generation, 0, 100).unwrap();
    assert!(!window.complete);
}

#[test]
fn an_unknown_repository_is_reported_as_missing() {
    let state = AppState::new();
    let generation = state.begin_graph();
    let result = state.build_graph(
        RepoId(999),
        &git_engine::CommitQuery::default(),
        generation,
        10,
        |_| true,
    );
    assert!(result.is_err());
}

fn search(state: &AppState, repo: RepoId, query: &git_engine::CommitQuery) -> Vec<Laid> {
    laid_out(state, repo, query, 100).0
}

#[test]
fn a_filter_narrows_the_stream() {
    let f = test_fixtures::linear(5).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let query = git_engine::CommitQuery {
        message: Some("commit 3".to_owned()),
        ..git_engine::CommitQuery::default()
    };
    let commits: Vec<String> = search(&state, repo, &query)
        .iter()
        .flat_map(|c| c.commits.iter().map(|r| r.summary.clone()))
        .collect();

    assert_eq!(commits, ["commit 3"]);
}

/// A filtered list draws a line only between a commit and a parent it also shows; a line
/// to a parent it does not show ends in an arrow, and never claims a lineage (R-161).
#[test]
fn a_filtered_result_links_what_it_shows_and_ends_the_rest_in_an_arrow() {
    let f = test_fixtures::linear(5).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let all: Vec<_> = stream(&state, repo, 100)
        .into_iter()
        .flat_map(|c| c.commits)
        .collect();

    let query = git_engine::CommitQuery {
        since: Some(all[2].timestamp),
        ..git_engine::CommitQuery::default()
    };
    let rows: Vec<_> = search(&state, repo, &query)
        .into_iter()
        .flat_map(|c| c.rows)
        .collect();

    assert_eq!(rows.len(), 3);
    let arrows = |row: &graph_engine::GraphRow| row.segments.iter().filter(|seg| seg.arrow).count();
    assert_eq!(arrows(&rows[0]), 0, "its parent is in the list");
    assert_eq!(arrows(&rows[1]), 0);
    assert_eq!(
        arrows(&rows[2]),
        1,
        "the oldest match's parent is filtered out"
    );
    assert!(rows.iter().all(|row| row.width == 1), "{rows:?}");
}

/// A shallow clone has no parents for its boundary commits. Their lines end in an arrow,
/// as a filtered list's do, rather than wait for a commit the walk never brings: column 0
/// ran on past HEAD's last commit to the bottom of the list.
#[test]
fn a_shallow_boundary_ends_its_lines_in_arrows() {
    let upstream = test_fixtures::linear(6).unwrap();
    upstream
        .git(&["switch", "-q", "-c", "side", "HEAD~3"])
        .unwrap();
    upstream.commit_file(10, "side.txt", "side\n").unwrap();
    upstream.git(&["switch", "-q", "main"]).unwrap();
    let clone = tempfile::tempdir().unwrap();
    let url = format!(
        "file://{}",
        upstream.path().to_string_lossy().replace('\\', "/")
    );
    let target = clone.path().join("shallow");
    let status = test_fixtures::git_command_in(clone.path())
        .args(["clone", "-q", "--depth", "2", "--no-single-branch", &url])
        .arg(&target)
        .status()
        .unwrap();
    assert!(status.success());
    let state = AppState::new();
    let repo = state.open_repository(&target).unwrap().repo;

    let chunks = stream(&state, repo, 100);
    let commits: Vec<_> = chunks.iter().flat_map(|c| c.commits.iter()).collect();
    let rows: Vec<_> = chunks.iter().flat_map(|c| c.rows.iter()).collect();
    let row_of = |summary: &str| {
        let at = commits.iter().position(|c| c.summary == summary).unwrap();
        rows[at]
    };

    for boundary in ["commit 4", "commit 2"] {
        let row = row_of(boundary);
        assert!(row.segments.iter().any(|s| s.arrow), "{boundary}: {row:?}");
    }
    let last_main = row_of("commit 4").row;
    for row in rows.iter().filter(|row| row.row > last_main) {
        assert!(
            row.segments.iter().all(|s| !s.primary || s.arrow),
            "{row:?}"
        );
    }
}

#[test]
fn rows_keep_counting_across_chunks_when_filtered() {
    let f = test_fixtures::linear(5).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let query = git_engine::CommitQuery {
        author: Some("fixture".to_owned()),
        ..git_engine::CommitQuery::default()
    };
    let (laid, progress) = laid_out(&state, repo, &query, 2);
    let rows: Vec<u32> = laid[0].rows.iter().map(|l| l.row).collect();

    assert!(progress.len() > 1, "more than one chunk: {progress:?}");
    assert_eq!(rows, [0, 1, 2, 3, 4]);
}

#[test]
fn an_empty_query_still_draws_the_graph() {
    let f = test_fixtures::diamond().unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let chunks = search(&state, repo, &git_engine::CommitQuery::default());

    assert!(
        chunks
            .iter()
            .any(|c| c.rows.iter().any(|row| row.width > 1))
    );
}

fn visible(names: &[&str]) -> git_engine::CommitQuery {
    git_engine::CommitQuery {
        visible_refs: Some(names.iter().map(|name| (*name).to_owned()).collect()),
        ..git_engine::CommitQuery::default()
    }
}

#[test]
fn narrowing_the_visible_refs_keeps_the_graph_a_graph() {
    let f = test_fixtures::diamond().unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let chunks = search(
        &state,
        repo,
        &visible(&["refs/heads/main", "refs/heads/dev"]),
    );

    assert!(
        chunks.iter().any(|c| c.rows.iter().any(|row| row.lane > 0)),
        "unticking a ref drops whole tips; the second lane of the diamond survives"
    );
}

#[test]
fn one_visible_ref_walks_only_its_own_history() {
    let f = test_fixtures::diamond().unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let summaries: Vec<String> = search(&state, repo, &visible(&["refs/heads/dev"]))
        .iter()
        .flat_map(|c| c.commits.iter().map(|r| r.summary.clone()))
        .collect();

    assert_eq!(summaries, ["commit 1", "commit 0"]);
}

#[test]
fn unticking_everything_shows_nothing() {
    let f = test_fixtures::diamond().unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let chunks = search(&state, repo, &visible(&[]));

    assert_eq!(chunks.iter().map(|c| c.commits.len()).sum::<usize>(), 0);
}

/// R-51 kept a filtered list flat; now it links whatever it shows (R-161), so a search
/// that every commit matches is simply the graph.
#[test]
fn a_search_every_commit_matches_draws_the_whole_graph() {
    let f = test_fixtures::diamond().unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let query = git_engine::CommitQuery {
        message: Some("commit".to_owned()),
        ..visible(&["refs/heads/main", "refs/heads/dev"])
    };
    let chunks = search(&state, repo, &query);

    assert!(
        chunks
            .iter()
            .any(|c| c.rows.iter().any(|row| row.width > 1))
    );
    assert!(
        chunks
            .iter()
            .flat_map(|c| &c.rows)
            .all(|row| row.segments.iter().all(|seg| !seg.arrow))
    );
}

// --- ticking refs: the walk that lost its reason to run stops (doc/12-risks.md, R-157) --

#[test]
fn a_newer_graph_request_retires_the_one_before() {
    let state = AppState::new();
    let first = state.begin_graph();
    let second = state.begin_graph();
    assert!(!state.is_current_graph(first));
    assert!(state.is_current_graph(second));
}

#[test]
fn a_tag_on_a_tree_is_reported_and_the_graph_is_drawn_without_it() {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["tag", "-a", "v-tree", "-m", "a tree", "HEAD^{tree}"])
        .unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let query = git_engine::CommitQuery {
        visible_refs: Some(vec!["HEAD".to_owned(), "refs/tags/v-tree".to_owned()]),
        ..git_engine::CommitQuery::default()
    };

    let generation = state.begin_graph();
    let skipped = state
        .build_graph(repo, &query, generation, 50, |_| true)
        .unwrap();
    let commits = state
        .graph_window(repo, generation, 0, 100)
        .unwrap()
        .commits
        .len();

    assert_eq!(commits, 2);
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].name, "refs/tags/v-tree");
}

/// The walk itself follows first parents (R-301); the merge draws its first line only (#26).
#[test]
fn first_parents_only_leave_the_merged_side_out_and_draw_one_line() {
    let f = test_fixtures::diamond().unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let query = git_engine::CommitQuery {
        view: git_engine::GraphView {
            first_parent: true,
            ..git_engine::GraphView::default()
        },
        ..visible(&["refs/heads/main", "HEAD"])
    };

    let chunks = search(&state, repo, &query);
    let summaries: Vec<&str> = chunks
        .iter()
        .flat_map(|c| c.commits.iter().map(|r| r.summary.as_str()))
        .collect();
    let rows: Vec<_> = chunks.iter().flat_map(|c| &c.rows).collect();

    assert_eq!(summaries, ["merge dev into main", "commit 2", "commit 0"]);
    assert!(rows.iter().all(|row| row.lane == 0 && row.width == 1));
    assert!(rows.iter().all(|row| row.segments.iter().all(|s| !s.arrow)));
    assert_eq!(chunks[0].commits[0].parents, [f.oid("main~1").unwrap()]);
}

/// Cutting long links holds the last rows back until the walk ends (R-330); they come
/// with the closing chunk, and every commit still gets its row, in order.
#[test]
fn rows_held_back_for_long_links_arrive_with_the_last_chunk() {
    let f = test_fixtures::linear(12).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let query = git_engine::CommitQuery {
        long_link_rows: Some(4),
        ..git_engine::CommitQuery::default()
    };

    let (laid, progress) = laid_out(&state, repo, &query, 5);

    let totals: Vec<u32> = progress.iter().map(|p| p.total).collect();
    assert!(progress.last().unwrap().is_last);
    assert_eq!(totals.last(), Some(&12));
    assert!(
        totals[totals.len() - 2] <= 12 - 4,
        "the lookahead is emptied at the end: {totals:?}"
    );
    let rows: Vec<u32> = laid[0].rows.iter().map(|row| row.row).collect();
    assert_eq!(rows, (0..12).collect::<Vec<_>>());
    assert!(!query.filters_rows(), "a layout option is not a filter");
}
