// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]
// The harness exists to print numbers; `print_stdout` is denied for production (INV-04).
#![allow(clippy::print_stdout)]

//! Budgets are looser than the product promises so a loaded machine does not go red. The
//! promise itself is held by the release benchmark; the 50 000-commit test that states it
//! runs on request (R-504). Numbers print with `--nocapture`.

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
        .search_graph(
            repo,
            &git_engine::CommitQuery::default(),
            DEFAULT_CHUNK_SIZE,
            |chunk| {
                if first_chunk.is_none() && !chunk.commits.is_empty() {
                    first_chunk = Some(streaming.elapsed());
                }
                rows += chunk.commits.len();
                true
            },
        )
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
#[ignore = "the promise is held on a release build by the benchmark, graph.full-layout · large \
            (doc/15-benchmark.md §5, R-504); a dev build has no margin for it"]
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
        .search_graph(
            repo,
            &git_engine::CommitQuery::default(),
            DEFAULT_CHUNK_SIZE,
            |chunk| {
                if first.is_none() && !chunk.commits.is_empty() {
                    first = Some(streaming.elapsed());
                }
                rows += chunk.commits.len();
                true
            },
        )
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

/// The walk takes parents and dates from the commit-graph file when there is one (R-302).
#[test]
fn fifty_thousand_commits_with_a_commit_graph_are_laid_out_in_the_budget() {
    let f = test_fixtures::stress(50_000).unwrap();
    f.git(&["commit-graph", "write", "--reachable"]).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let generation = state.begin_graph();
    let started = Instant::now();
    let mut rows = 0;
    state
        .build_graph(
            repo,
            &CommitQuery::default(),
            generation,
            DEFAULT_CHUNK_SIZE,
            |p| {
                rows = p.total;
                true
            },
        )
        .unwrap();
    let walked = started.elapsed();
    let window = Instant::now();
    let shown = state.graph_window(repo, generation, 0, 128).unwrap();
    let first_window = window.elapsed();
    // Far down, straight away: what a jump costs before the texts are read ahead (R-303).
    let cold = Instant::now();
    state.graph_window(repo, generation, 40_000, 128).unwrap();
    let cold = cold.elapsed();
    let until = Instant::now() + Duration::from_secs(20);
    while state.graph_texts_read(repo) < 50_000 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    println!(
        "50k: texts read ahead after    {:>7} ms",
        started.elapsed().as_millis()
    );
    let warm = Instant::now();
    state.graph_window(repo, generation, 30_000, 128).unwrap();
    let warm = warm.elapsed();

    assert_eq!(rows, 50_000);
    assert_eq!(shown.commits[0].summary, "commit 49999");
    assert_eq!(state.graph_texts_read(repo), 50_000);
    report(
        "50k + commit-graph: laid out",
        walked,
        Duration::from_millis(500),
    );
    report(
        "50k + commit-graph: first window",
        first_window,
        Duration::from_millis(100),
    );
    println!("50k: a far window, texts unread {:>7} us", cold.as_micros());
    println!("50k: a far window, read ahead   {:>7} us", warm.as_micros());
    assert!(warm < Duration::from_millis(100) && cold < Duration::from_millis(100));
}

/// Ticking a ref re-lays the graph from the rows the last walk read (R-301).
#[test]
fn fifty_thousand_commits_are_laid_out_again_from_the_last_graph() {
    let f = test_fixtures::stress(50_000).unwrap();
    f.git(&["branch", "side", "HEAD~25000"]).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let walk = |query: &CommitQuery| {
        let generation = state.begin_graph();
        let started = Instant::now();
        let mut rows = 0;
        state
            .build_graph(repo, query, generation, DEFAULT_CHUNK_SIZE, |p| {
                rows = p.total;
                true
            })
            .unwrap();
        (started.elapsed(), rows)
    };

    let (full, _) = walk(&CommitQuery::default());
    let (again, rows) = walk(&CommitQuery {
        visible_refs: Some(vec!["refs/heads/main".to_owned(), "HEAD".to_owned()]),
        ..CommitQuery::default()
    });

    assert_eq!(rows, 50_000);
    println!("50k: first walk {:>7} ms", full.as_millis());
    report(
        "50k: walked again after a tick",
        again,
        Duration::from_millis(500),
    );
}

#[test]
fn status_on_a_large_repository_stays_inside_its_budget() {
    let f = test_fixtures::stress(COMMITS).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    // Warm: the first call pays for the index and the object cache, and the panel that
    // depends on this is never the first thing drawn.
    let _ = state.repo_status(repo).unwrap();

    let started = Instant::now();
    let status = state.repo_status(repo).unwrap();
    report("repo_status", started.elapsed(), Duration::from_millis(500));

    assert_eq!(status.staged + status.unstaged + status.untracked, 0);
}
