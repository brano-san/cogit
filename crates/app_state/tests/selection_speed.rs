// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]
// The harness exists to print numbers; `print_stdout` is denied for production (INV-04).
#![allow(clippy::print_stdout)]

//! Clicking a commit must feel instant. Both halves of the answer are read through `gix`,
//! so the only way to spend half a second is to ask for more work than the user can see.

use app_state::AppState;
use std::time::{Duration, Instant};

#[test]
fn selecting_a_commit_in_a_broad_tree_answers_within_the_budget() {
    let f = test_fixtures::wide(4000).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let started = Instant::now();
    let details = state.commit_details(repo, "HEAD").unwrap();
    let files = state.commit_files(repo, "HEAD").unwrap();
    let elapsed = started.elapsed();

    println!("select over 4000 files {:>6} ms", elapsed.as_millis());
    assert_eq!(details.summary, "touch one file");
    assert_eq!(files.len(), 1);
    assert!(
        elapsed < Duration::from_millis(120),
        "selecting a commit took {elapsed:?}"
    );
}
