// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]
// The harness exists to print numbers; `print_stdout` is denied for production (INV-04).
#![allow(clippy::print_stdout)]

//! Budgets are looser than the product promises so a loaded machine does not go red; the
//! 50 000-commit test asserts the promise itself. Numbers print with `--nocapture`.

use app_state::{AppState, DEFAULT_CHUNK_SIZE};
use diff_engine::DiffOptions;
use git_engine::{CommitQuery, DiffSpec};
use std::time::{Duration, Instant};

const COMMITS: i64 = 10_000;

fn report(label: &str, elapsed: Duration, budget: Duration) {
    println!(
        "{label:<34} {:>7} ms   (budget {} ms)",
        elapsed.as_millis(),
        budget.as_millis()
    );
    assert!(
        elapsed < budget,
        "{label} took {elapsed:?}, budget is {budget:?}"
    );
}

#[test]
fn a_ten_thousand_commit_repository_opens_and_streams() {
    let built = Instant::now();
    let f = test_fixtures::stress(COMMITS).unwrap();
    println!(
        "fixture of {COMMITS} commits        {:>7} ms",
        built.elapsed().as_millis()
    );

    let state = AppState::new();

    let opening = Instant::now();
    let summary = state.open_repository(f.path()).unwrap();
    report("open_repository", opening.elapsed(), Duration::from_secs(2));
    let repo = summary.repo;

    let mut first_chunk = None;
    let mut rows = 0_usize;
    let streaming = Instant::now();
    state
        .stream_graph(repo, DEFAULT_CHUNK_SIZE, |chunk| {
            if first_chunk.is_none() && !chunk.commits.is_empty() {
                first_chunk = Some(streaming.elapsed());
            }
            rows += chunk.commits.len();
            true
        })
        .unwrap();

    assert_eq!(rows, COMMITS as usize);
    report(
        "first screen of 200 commits",
        first_chunk.unwrap(),
        Duration::from_millis(900),
    );
    report(
        "full graph with lane layout",
        streaming.elapsed(),
        Duration::from_secs(15),
    );
}

#[test]
fn a_filtered_search_answers_quickly_on_a_large_history() {
    let f = test_fixtures::stress(COMMITS).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let query = CommitQuery {
        path: Some("data.txt".to_owned()),
        ..CommitQuery::default()
    };

    let mut first = None;
    let started = Instant::now();
    state
        .search_graph(repo, &query, DEFAULT_CHUNK_SIZE, |chunk| {
            if first.is_none() && !chunk.commits.is_empty() {
                first = Some(started.elapsed());
            }
            // The first screen is what the user waits for; the rest streams behind it.
            first.is_none()
        })
        .unwrap();

    report(
        "first screen of a path filter",
        first.expect("the stress fixture rewrites one file every commit"),
        Duration::from_secs(3),
    );
}

#[test]
fn reading_one_commit_from_a_large_history_stays_instant() {
    let f = test_fixtures::stress(COMMITS).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let details = Instant::now();
    let head = state.commit_details(repo, "HEAD").unwrap();
    report(
        "commit_details",
        details.elapsed(),
        Duration::from_millis(500),
    );

    let files = Instant::now();
    let changed = state.commit_files(repo, "HEAD").unwrap();
    report("commit_files", files.elapsed(), Duration::from_millis(500));
    assert!(!changed.is_empty());

    let diffing = Instant::now();
    let diff = state
        .diff_file(
            repo,
            &DiffSpec::CommitVsParent { oid: head.oid },
            &changed[0].path,
            &DiffOptions::default(),
        )
        .unwrap();
    report("diff_file", diffing.elapsed(), Duration::from_millis(500));
    assert!(!matches!(diff, diff_engine::FileDiff::Unchanged));
}

#[test]
fn fifty_thousand_commits_meet_the_product_promise() {
    let built = Instant::now();
    let f = test_fixtures::stress(50_000).unwrap();
    println!(
        "fixture of 50000 commits        {:>7} ms",
        built.elapsed().as_millis()
    );

    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let mut first = None;
    let mut rows = 0_usize;
    let streaming = Instant::now();
    state
        .stream_graph(repo, DEFAULT_CHUNK_SIZE, |chunk| {
            if first.is_none() && !chunk.commits.is_empty() {
                first = Some(streaming.elapsed());
            }
            rows += chunk.commits.len();
            true
        })
        .unwrap();

    assert_eq!(rows, 50_000);
    report(
        "50k: first 200 on screen",
        first.unwrap(),
        Duration::from_millis(300),
    );
    report(
        "50k: whole graph laid out",
        streaming.elapsed(),
        Duration::from_millis(500),
    );
}
