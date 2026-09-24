// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! A walk that copies the rows of the last graph lays out exactly what a fresh walk would
//! (R-301), and says how many leading rows it left as they were.

use app_state::{AppState, GraphProgress, RepoId};
use git_engine::CommitQuery;
use graph_engine::GraphRow;

type Laid = Vec<(String, GraphRow)>;

fn build(state: &AppState, repo: RepoId, query: &CommitQuery) -> (u32, Vec<GraphProgress>, Laid) {
    let generation = state.begin_graph();
    let mut progress = Vec::new();
    state
        .build_graph(repo, query, generation, 3, |p| {
            progress.push(p);
            true
        })
        .unwrap();
    let window = state.graph_window(repo, generation, 0, 10_000).unwrap();
    let laid = window
        .commits
        .into_iter()
        .map(|c| c.oid)
        .zip(window.rows)
        .collect();
    (generation, progress, laid)
}

fn fresh(f: &test_fixtures::Fixture, query: &CommitQuery) -> Laid {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    build(&state, repo, query).2
}

fn ticked(refs: &[&str]) -> CommitQuery {
    CommitQuery {
        visible_refs: Some(refs.iter().map(|r| (*r).to_owned()).collect()),
        ..CommitQuery::default()
    }
}

/// Branches forking and merging, two of them committed in the same second.
fn forked() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(4).unwrap();
    f.git(&["switch", "-c", "feat", "main~2"]).unwrap();
    f.commit_file(5, "f1.txt", "f1\n").unwrap();
    f.commit_file(6, "f2.txt", "f2\n").unwrap();
    f.git(&["switch", "-c", "side", "main"]).unwrap();
    f.commit_file(5, "s1.txt", "s1\n").unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.commit_file(7, "m.txt", "m\n").unwrap();
    f.merge(8, &["side"], "merge side").unwrap();
    f.git(&["branch", "old", "main~3"]).unwrap();
    f
}

#[test]
fn every_tick_lays_out_what_a_fresh_walk_would() {
    let f = forked();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let queries = [
        CommitQuery::default(),
        ticked(&["refs/heads/main", "HEAD"]),
        ticked(&["refs/heads/feat"]),
        ticked(&["refs/heads/feat", "refs/heads/main", "HEAD"]),
        ticked(&["refs/heads/old"]),
        CommitQuery::default(),
    ];

    for query in &queries {
        let (_, _, laid) = build(&state, repo, query);
        assert_eq!(laid, fresh(&f, query), "{:?}", query.visible_refs);
    }
}

/// Rows held back to see how far their links reach (R-330) come out the same too, and a
/// new threshold lays the copied commits out again instead of serving the old rows.
#[test]
fn ticks_and_thresholds_with_long_links_cut_lay_out_what_a_fresh_walk_would() {
    let f = forked();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let cut = |rows: Option<u32>, query: CommitQuery| CommitQuery {
        long_link_rows: rows,
        ..query
    };
    let queries = [
        cut(Some(2), CommitQuery::default()),
        cut(
            Some(2),
            ticked(&["refs/heads/feat", "refs/heads/main", "HEAD"]),
        ),
        cut(
            Some(1),
            ticked(&["refs/heads/feat", "refs/heads/main", "HEAD"]),
        ),
        cut(
            None,
            ticked(&["refs/heads/feat", "refs/heads/main", "HEAD"]),
        ),
        cut(Some(3), CommitQuery::default()),
    ];

    for query in &queries {
        let (_, _, laid) = build(&state, repo, query);
        assert_eq!(laid, fresh(&f, query), "{query:?}");
    }
}

#[test]
fn a_tick_with_long_links_cut_keeps_only_rows_that_are_final_and_equal() {
    let f = forked();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let cut = |refs: &[&str]| CommitQuery {
        long_link_rows: Some(2),
        ..ticked(refs)
    };
    let (before, _, old) = build(&state, repo, &cut(&["refs/heads/main", "HEAD"]));

    let (_, progress, new) = build(
        &state,
        repo,
        &cut(&["refs/heads/main", "HEAD", "refs/heads/feat"]),
    );

    let last = progress.last().unwrap();
    assert_eq!(last.base, Some(before));
    let kept = usize::try_from(last.kept).unwrap();
    assert!(kept < new.len());
    assert_eq!(new[..kept], old[..kept]);
    assert!(
        progress.iter().all(|p| p.kept <= p.total),
        "never more kept than laid out"
    );
}

#[test]
fn commits_made_outside_are_read_and_the_rest_copied_the_same_as_fresh() {
    let f = forked();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    build(&state, repo, &CommitQuery::default());

    f.git(&["switch", "feat"]).unwrap();
    f.commit_file(9, "f3.txt", "f3\n").unwrap();
    f.merge(10, &["main"], "merge main into feat").unwrap();
    let (_, _, laid) = build(&state, repo, &CommitQuery::default());

    assert_eq!(laid, fresh(&f, &CommitQuery::default()));
    assert_eq!(laid.len(), 11);
}

#[test]
fn a_tick_says_which_leading_rows_stayed_as_they_were() {
    let f = forked();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let main = ticked(&["refs/heads/main", "HEAD"]);
    let (before, _, old) = build(&state, repo, &main);

    let (_, progress, new) = build(
        &state,
        repo,
        &ticked(&["refs/heads/main", "HEAD", "refs/heads/feat"]),
    );

    let last = progress.last().unwrap();
    assert_eq!(last.base, Some(before));
    let kept = usize::try_from(last.kept).unwrap();
    assert!(kept > 0 && kept < new.len(), "kept {kept} of {}", new.len());
    assert_eq!(new[..kept], old[..kept]);
    assert_ne!(new.get(kept), old.get(kept));
}

#[test]
fn the_graph_being_replaced_is_still_read_while_the_new_one_is_walked() {
    let f = forked();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let (before, _, old) = build(&state, repo, &CommitQuery::default());

    let generation = state.begin_graph();
    let mut during = None;
    state
        .build_graph(repo, &ticked(&["refs/heads/feat"]), generation, 2, |_| {
            if during.is_none() {
                during = state
                    .graph_window(repo, before, 0, 100)
                    .map(|w| w.rows.len());
            }
            true
        })
        .unwrap();

    assert_eq!(during, Some(old.len()));
    assert!(
        state.graph_window(repo, before, 0, 100).is_none(),
        "gone once the new graph is complete"
    );
}
