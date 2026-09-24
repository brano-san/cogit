// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]
// The harness exists to print numbers; `print_stdout` is denied for production (INV-04).
#![allow(clippy::print_stdout)]

//! Staging must feel instant. Every `git` invocation costs 5-30 ms of process spawn on
//! Windows, so the budget is really a budget on how many processes an action starts.

use app_state::AppState;
use std::time::{Duration, Instant};

fn report(label: &str, elapsed: Duration, budget: Duration) {
    println!(
        "{label:<26} {:>6} ms   (budget {} ms)",
        elapsed.as_millis(),
        budget.as_millis()
    );
    assert!(elapsed < budget, "{label} took {elapsed:?}");
}

#[test]
fn staging_a_file_is_a_single_git_invocation() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    std::fs::write(f.path().join("file0.txt"), "edited\n").unwrap();
    state.clear_command_log();

    let started = Instant::now();
    state.stage_paths(repo, &["file0.txt".to_owned()]).unwrap();
    report("stage", started.elapsed(), Duration::from_millis(400));

    assert_eq!(state.command_log().len(), 1, "{:?}", state.command_log());
}

#[test]
fn staging_everything_is_one_git_add_without_a_path_list() {
    let f = test_fixtures::linear(3).unwrap();
    for name in ["file0.txt", "file1.txt", "fresh.txt"] {
        std::fs::write(
            f.path().join(name),
            "edited
",
        )
        .unwrap();
    }
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    state.clear_command_log();

    let started = Instant::now();
    state.stage_all(repo, 3).unwrap();
    report("stage all", started.elapsed(), Duration::from_millis(400));

    let log = state.command_log();
    let commands: Vec<&str> = log.iter().map(|entry| entry.command.as_str()).collect();
    assert_eq!(commands, ["git add --all"]);
}

#[test]
fn unstaging_a_file_is_a_single_git_invocation() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    std::fs::write(f.path().join("file0.txt"), "edited\n").unwrap();
    state.stage_paths(repo, &["file0.txt".to_owned()]).unwrap();
    state.clear_command_log();

    let started = Instant::now();
    state
        .unstage_paths(repo, &["file0.txt".to_owned()])
        .unwrap();
    report("unstage", started.elapsed(), Duration::from_millis(400));

    assert_eq!(state.command_log().len(), 1, "{:?}", state.command_log());
}

#[test]
fn discarding_a_file_is_a_single_git_invocation() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    std::fs::write(f.path().join("file0.txt"), "edited\n").unwrap();
    state.clear_command_log();

    let started = Instant::now();
    state
        .discard_paths(repo, &["file0.txt".to_owned()])
        .unwrap();
    report("discard", started.elapsed(), Duration::from_millis(400));

    let log = state.command_log();
    assert_eq!(
        log.len(),
        1,
        "the stash itself is the only process needed, got {:?}",
        log.iter().map(|e| &e.command).collect::<Vec<_>>()
    );
}

#[test]
fn reading_the_working_tree_spawns_no_process_at_all() {
    let f = test_fixtures::linear(3).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    std::fs::write(f.path().join("file0.txt"), "edited\n").unwrap();
    state.clear_command_log();

    let started = Instant::now();
    state
        .worktree_files(repo, git_engine::WorktreeView::default())
        .unwrap();
    report(
        "worktree_files",
        started.elapsed(),
        Duration::from_millis(200),
    );

    assert!(
        state.command_log().is_empty(),
        "reads go through gix, not the CLI"
    );
}

#[test]
fn a_status_refresh_skips_the_refs_a_full_open_reads() {
    let f = test_fixtures::linear(2).unwrap();
    for i in 0..200 {
        f.git(&["branch", &format!("topic-{i}")]).unwrap();
    }
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let full = Instant::now();
    let summary = state.open_repository(f.path()).unwrap();
    let full = full.elapsed();

    let quick = Instant::now();
    state.repo_status(repo).unwrap();
    let quick = quick.elapsed();

    println!(
        "open_repository {:>6} us  ({} refs)",
        full.as_micros(),
        summary.branches.len()
    );
    println!("repo_status     {:>6} us", quick.as_micros());
    assert!(summary.branches.len() >= 200);
    assert!(
        quick * 2 < full,
        "reading 200 refs to learn the staged count is waste: {quick:?} vs {full:?}"
    );
}
