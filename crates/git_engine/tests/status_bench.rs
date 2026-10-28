//! Timing of what one refresh pass asks of the status: the file list and the counters
//! (R-316). `COGIT_BENCH_REPO=<folder> cargo test -p git_engine --release --test status_bench
//! -- --ignored --nocapture`. The old code is measured with `files.state` replaced by a
//! second `working_state()` call (doc/15-benchmark.md).
#![allow(clippy::unwrap_used, clippy::print_stdout)]

use git_engine::RepoHandle;
use std::time::{Duration, Instant};

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

#[test]
#[ignore = "needs COGIT_BENCH_REPO"]
fn pass_status_reads() {
    let Ok(path) = std::env::var("COGIT_BENCH_REPO") else {
        return;
    };
    let handle = RepoHandle::open_exact(std::path::Path::new(&path)).unwrap();
    let mut pass = Vec::new();
    for round in 0..14 {
        let started = Instant::now();
        let files = handle.worktree_files().unwrap();
        let counted = files.state.status;
        if round >= 4 {
            pass.push(started.elapsed());
        }
        assert!(counted.staged + counted.unstaged + counted.untracked > 0);
    }
    println!("{path}: list + counters {:?}", median(pass));
}

#[test]
#[ignore = "needs COGIT_BENCH_FRESH"]
fn status_rounds_of_a_fresh_clone() {
    let Ok(path) = std::env::var("COGIT_BENCH_FRESH") else {
        return;
    };
    let handle = RepoHandle::open_exact(std::path::Path::new(&path)).unwrap();
    for round in 0..4 {
        let started = Instant::now();
        let files = handle.worktree_files().unwrap();
        println!(
            "{path} round {round}: {:?} ({} entries)",
            started.elapsed(),
            files.unstaged.len()
        );
    }
}

#[test]
#[ignore = "needs COGIT_BENCH_COMMIT"]
fn files_of_a_big_commit() {
    let Ok(path) = std::env::var("COGIT_BENCH_COMMIT") else {
        return;
    };
    let handle = RepoHandle::open_exact(std::path::Path::new(&path)).unwrap();
    let mut samples = Vec::new();
    for _ in 0..5 {
        let started = Instant::now();
        let files = handle.commit_files("HEAD").unwrap();
        samples.push(started.elapsed());
        assert!(!files.is_empty());
    }
    let files = handle.commit_files("HEAD").unwrap();
    let renamed = files.iter().filter(|file| file.old_path.is_some()).count();
    println!(
        "{path}: {} files, {renamed} renamed or copied, median {:?}",
        files.len(),
        median(samples)
    );
}
