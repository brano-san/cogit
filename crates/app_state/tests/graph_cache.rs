// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! One graph per repository survives switching between them (R-300).

use app_state::{AppState, GraphProgress, RepoId};
use git_engine::CommitQuery;

fn build_with(state: &AppState, repo: RepoId, query: &CommitQuery) -> (u32, Vec<GraphProgress>) {
    let generation = state.begin_graph();
    let mut progress = Vec::new();
    state
        .build_graph(repo, query, generation, 4, |p| {
            progress.push(p);
            true
        })
        .unwrap();
    (generation, progress)
}

fn build(state: &AppState, repo: RepoId) -> (u32, Vec<GraphProgress>) {
    build_with(state, repo, &CommitQuery::default())
}

fn totals(progress: &[GraphProgress]) -> Vec<u32> {
    progress.iter().map(|p| p.total).collect()
}

fn summaries(state: &AppState, repo: RepoId, generation: u32) -> Vec<String> {
    state
        .graph_window(repo, generation, 0, 100)
        .unwrap()
        .commits
        .into_iter()
        .map(|c| c.summary)
        .collect()
}

#[test]
fn switching_back_answers_from_the_cache_without_a_walk() {
    let a = test_fixtures::linear(10).unwrap();
    let b = test_fixtures::linear(3).unwrap();
    let state = AppState::new();
    let ra = state.open_repository(a.path()).unwrap().repo;
    let rb = state.open_repository(b.path()).unwrap().repo;

    let (_, first) = build(&state, ra);
    build(&state, rb);
    let (generation, again) = build(&state, ra);

    assert_eq!(totals(&first), [4, 8, 10, 10]);
    assert_eq!(
        totals(&again),
        [10],
        "one message, the whole graph, no chunks"
    );
    assert!(again[0].is_last && again[0].generation == generation);
    assert_eq!(summaries(&state, ra, generation).len(), 10);
}

#[test]
fn the_graph_on_screen_stays_readable_while_another_is_built() {
    let a = test_fixtures::linear(5).unwrap();
    let b = test_fixtures::linear(3).unwrap();
    let state = AppState::new();
    let ra = state.open_repository(a.path()).unwrap().repo;
    let rb = state.open_repository(b.path()).unwrap().repo;

    let (shown, _) = build(&state, ra);
    build(&state, rb);

    assert_eq!(summaries(&state, ra, shown).len(), 5);
}

#[test]
fn refs_changed_outside_while_away_are_in_the_graph_after_switching_back() {
    let a = test_fixtures::linear(4).unwrap();
    let b = test_fixtures::linear(2).unwrap();
    let state = AppState::new();
    let ra = state.open_repository(a.path()).unwrap().repo;
    let rb = state.open_repository(b.path()).unwrap().repo;
    build(&state, ra);
    build(&state, rb);

    // Git from a terminal while B is on screen; nothing tells the state about it.
    a.commit_file(10, "outside.txt", "outside\n").unwrap();
    a.git(&["branch", "side", "HEAD~2"]).unwrap();
    let (generation, progress) = build(&state, ra);

    assert_eq!(progress.last().unwrap().total, 5);
    assert_eq!(summaries(&state, ra, generation)[0], "commit 10");
}

#[test]
fn a_ref_moved_back_outside_is_noticed_too() {
    let a = test_fixtures::linear(4).unwrap();
    let state = AppState::new();
    let ra = state.open_repository(a.path()).unwrap().repo;
    build(&state, ra);

    a.git(&["reset", "--hard", "HEAD~1"]).unwrap();
    let (generation, progress) = build(&state, ra);

    assert_eq!(progress.last().unwrap().total, 3);
    assert_eq!(summaries(&state, ra, generation)[0], "commit 2");
}

#[test]
fn another_query_is_walked_not_served() {
    let a = test_fixtures::linear(6).unwrap();
    let state = AppState::new();
    let ra = state.open_repository(a.path()).unwrap().repo;
    build(&state, ra);

    let query = CommitQuery {
        message: Some("commit 1".to_owned()),
        ..CommitQuery::default()
    };
    let (generation, _) = build_with(&state, ra, &query);

    assert_eq!(summaries(&state, ra, generation), ["commit 1"]);
}

#[test]
fn closing_a_repository_drops_its_graph() {
    let a = test_fixtures::linear(3).unwrap();
    let state = AppState::new();
    let ra = state.open_repository(a.path()).unwrap().repo;
    let (generation, _) = build(&state, ra);

    state.close_repository(ra);

    assert!(state.graph_window(ra, generation, 0, 10).is_none());
}

/// Editing `.mailmap` renames the rows on screen; the layout is not walked again (R-302).
#[test]
fn a_new_mailmap_renames_the_cached_rows_without_a_walk() {
    let a = test_fixtures::linear(6).unwrap();
    let state = AppState::new();
    let ra = state.open_repository(a.path()).unwrap().repo;
    let (first, _) = build(&state, ra);
    let before = state.graph_window(ra, first, 0, 1).unwrap().commits[0].clone();

    std::fs::write(
        a.path().join(".mailmap"),
        format!(
            "Someone Else <else@example.test> <{}>\n",
            before.author_email
        ),
    )
    .unwrap();
    let (generation, progress) = build(&state, ra);

    assert_eq!(totals(&progress), [6], "one message, no walk");
    assert_eq!(
        progress[0].kept, 0,
        "blocks fetched with the old names are not kept"
    );
    let after = &state.graph_window(ra, generation, 0, 1).unwrap().commits[0];
    assert_eq!(
        (after.author_name.as_str(), after.author_email.as_str()),
        ("Someone Else", "else@example.test")
    );
    assert_eq!(after.summary, before.summary);
}

/// A filter by author matches through the mailmap, so a new one walks again.
#[test]
fn a_new_mailmap_walks_an_author_filter_again() {
    let a = test_fixtures::linear(4).unwrap();
    let state = AppState::new();
    let ra = state.open_repository(a.path()).unwrap().repo;
    let query = CommitQuery {
        author: Some("Someone Else".to_owned()),
        ..CommitQuery::default()
    };
    let (_, progress) = build_with(&state, ra, &query);
    assert_eq!(progress.last().unwrap().total, 0);
    let email = a
        .git(&["log", "-1", "--format=%ae"])
        .unwrap()
        .trim()
        .to_owned();

    std::fs::write(
        a.path().join(".mailmap"),
        format!("Someone Else <else@example.test> <{email}>\n"),
    )
    .unwrap();
    let (_, progress) = build_with(&state, ra, &query);

    assert_eq!(progress.last().unwrap().total, 4);
}
