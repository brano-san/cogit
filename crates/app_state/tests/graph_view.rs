// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, GraphChunk};
use git_engine::{CommitQuery, GraphView};

fn search(state: &AppState, repo: app_state::RepoId, query: &CommitQuery) -> Vec<GraphChunk> {
    let mut chunks = Vec::new();
    state
        .search_graph(repo, query, 100, |chunk| {
            chunks.push(chunk);
            true
        })
        .unwrap();
    chunks
}

fn first_parent(refs: Option<&[&str]>) -> CommitQuery {
    CommitQuery {
        visible_refs: refs.map(|refs| refs.iter().map(|r| (*r).to_owned()).collect()),
        view: GraphView { first_parent: true },
        ..CommitQuery::default()
    }
}

#[test]
fn first_parent_shows_one_line_without_the_merged_branch() {
    let f = test_fixtures::diamond().unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let chunks = search(&state, repo, &first_parent(Some(&["refs/heads/main"])));
    let commits: Vec<&str> = chunks
        .iter()
        .flat_map(|c| c.commits.iter().map(|c| c.oid.as_str()))
        .collect();
    let main: Vec<String> = ["main", "main~1", "main~2"]
        .iter()
        .map(|rev| f.oid(rev).unwrap())
        .collect();

    assert_eq!(commits, main);
    assert!(
        chunks
            .iter()
            .flat_map(|c| &c.rows)
            .all(|row| row.width == 1)
    );
    assert_eq!(chunks[0].commits[0].parents, [main[1].clone()]);
}

#[test]
fn first_parent_keeps_a_ticked_branch_the_main_line_merged() {
    let f = test_fixtures::diamond().unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let chunks = search(&state, repo, &first_parent(None));
    let rows: usize = chunks.iter().map(|c| c.rows.len()).sum();
    assert_eq!(
        rows, 4,
        "dev is a ref the walk starts from, so its line stays"
    );
}

#[test]
fn a_filtered_list_ignores_the_view() {
    let f = test_fixtures::diamond().unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let query = CommitQuery {
        message: Some("commit".to_owned()),
        ..first_parent(Some(&["refs/heads/main"]))
    };
    let rows: usize = search(&state, repo, &query)
        .iter()
        .map(|c| c.rows.len())
        .sum();
    assert_eq!(
        rows, 3,
        "commit 0, 1 and 2 match, whatever line they are on"
    );
}
