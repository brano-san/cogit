// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stderr)]

//! INV-02 sends a list longer than ~500 over a `Channel`. The lists below go in one answer
//! instead (R-511); this measures what that answer costs at ten thousand entries — its JSON,
//! which Tauri builds with `serde_json` and the webview parses whole.

use app_state::{AppState, RepoId};
use std::time::{Duration, Instant};

const ENTRIES: i64 = 10_000;
/// One frame, where our crates are optimised (release; `CARGO_PROFILE_DEV_OPT_LEVEL=2`).
/// The suite's dev profile leaves them unoptimised and measures about ten times slower.
const FRAME: Duration = Duration::from_millis(if cfg!(debug_assertions) { 64 } else { 16 });
/// About 190 bytes a blame line; more means a field grew into every entry.
const MOST_BYTES: usize = 2 << 20;

fn open(f: &test_fixtures::Fixture) -> (AppState, RepoId) {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    (state, repo)
}

/// The size of the answer and the median of five serialisations of it.
fn cost<T: serde::Serialize>(value: &T) -> (usize, Duration) {
    let mut runs: Vec<Duration> = (0..5)
        .map(|_| {
            let started = Instant::now();
            let text = serde_json::to_string(value).unwrap();
            let took = started.elapsed();
            assert!(!text.is_empty());
            took
        })
        .collect();
    runs.sort();
    (serde_json::to_string(value).unwrap().len(), runs[2])
}

/// The measurement as a line, `Err` when it is over budget.
fn check(name: &str, entries: usize, (bytes, took): (usize, Duration)) -> Result<String, String> {
    assert!(
        entries >= usize::try_from(ENTRIES).unwrap(),
        "{name}: only {entries} entries"
    );
    let line = format!(
        "{name}: {entries} entries, {} KiB, serialised in {:.2} ms",
        bytes / 1024,
        took.as_secs_f64() * 1000.0
    );
    if took >= FRAME || bytes > MOST_BYTES {
        Err(line)
    } else {
        Ok(line)
    }
}

#[test]
fn ten_thousand_entries_in_one_answer_stay_within_a_frame_budget() {
    let f = test_fixtures::wide(ENTRIES).unwrap();
    let text: String = (0..ENTRIES)
        .map(|n| format!("line {n} of a long file\n"))
        .collect();
    f.commit_file(5, "long.txt", &text).unwrap();
    let seed = f.oid("HEAD~2").unwrap();
    let (state, repo) = open(&f);

    let mut measured = Vec::new();
    let tree = state.tree_files(repo, "HEAD").unwrap();
    measured.push(check("commit_tree_files", tree.len(), cost(&tree)));

    let files = state.commit_files(repo, &seed).unwrap();
    measured.push(check("commit_files", files.len(), cost(&files)));

    let blame = state.blame(repo, "long.txt", "HEAD").unwrap();
    measured.push(check("blame", blame.len(), cost(&blame)));

    f.git(&["rm", "-r", "-q", "--cached", "."]).unwrap();
    let worktree = state
        .worktree_files(repo, git_engine::WorktreeView::default())
        .unwrap();
    let listed = worktree.staged.len().max(worktree.unstaged.len());
    measured.push(check("worktree_files", listed, cost(&worktree)));

    for line in &measured {
        eprintln!("{}", line.as_ref().unwrap_or_else(|over| over));
    }
    let over: Vec<&String> = measured
        .iter()
        .filter_map(|line| line.as_ref().err())
        .collect();
    assert!(over.is_empty(), "over budget: {over:?}");
}
