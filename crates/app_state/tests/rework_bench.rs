//! Timing harnesses for the rework-2 benchmarks (doc/15-benchmark.md), run on request with
//! `COGIT_BENCH_REPO=<large> COGIT_BENCH_DIRTY=<dirty> cargo test -p app_state --release
//! --test rework_bench -- --ignored --nocapture`. They use only what the old and the new
//! code both have, so the same file measures both.
#![allow(clippy::unwrap_used, clippy::print_stdout)]

use app_state::graph_overlay::{GraphPaintRequest, PaintTip};
use app_state::{AppState, DEFAULT_CHUNK_SIZE};
use git_engine::{CommitQuery, WorktreeView};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

fn tips(path: &str) -> Vec<PaintTip> {
    let listing = test_fixtures::git_command_in(Path::new(path))
        .args(["for-each-ref", "--format=%(objectname)", "refs/heads"])
        .output()
        .unwrap();
    String::from_utf8(listing.stdout)
        .unwrap()
        .lines()
        .take(20)
        .enumerate()
        .map(|(slot, oid)| PaintTip {
            oid: oid.to_owned(),
            slot: u8::try_from(slot % 8).unwrap(),
        })
        .collect()
}

#[derive(Clone, Copy)]
enum Poll {
    Nothing,
    Window,
    Overlay,
}

/// One full walk with a reader asking for the first screen over and over, as the panel does
/// while chunks arrive. Returns the walk and the number of answers it was given.
fn walk(path: &str, poll: Poll, request: &GraphPaintRequest) -> (Duration, u32) {
    let state = AppState::new();
    let repo = state.open_repository(Path::new(path)).unwrap().repo;
    let generation = state.begin_graph();
    let stop = AtomicBool::new(false);
    let answers = AtomicU32::new(0);
    let mut elapsed = Duration::ZERO;
    std::thread::scope(|scope| {
        scope.spawn(|| {
            while !stop.load(Ordering::Relaxed) {
                match poll {
                    Poll::Nothing => break,
                    Poll::Window => {
                        std::hint::black_box(state.graph_window(repo, generation, 0, 128));
                    }
                    Poll::Overlay => {
                        std::hint::black_box(
                            state.graph_overlay(repo, generation, 0, 200, request),
                        );
                    }
                }
                answers.fetch_add(1, Ordering::Relaxed);
            }
        });
        let started = Instant::now();
        state
            .build_graph(
                repo,
                &CommitQuery::default(),
                generation,
                DEFAULT_CHUNK_SIZE,
                |_| true,
            )
            .unwrap();
        elapsed = started.elapsed();
        stop.store(true, Ordering::Relaxed);
    });
    (elapsed, answers.load(Ordering::Relaxed))
}

/// GH-01 and GH-02: the walk of the large history while the panel reads it.
#[test]
#[ignore = "needs COGIT_BENCH_REPO"]
fn gh01_gh02_a_walk_beside_its_readers() {
    let Ok(path) = std::env::var("COGIT_BENCH_REPO") else {
        return;
    };
    let request = GraphPaintRequest {
        tips: tips(&path),
        ..Default::default()
    };
    for (label, poll) in [
        ("no reader", Poll::Nothing),
        ("window reader (GH-01)", Poll::Window),
        ("overlay reader, 20 tips ticked (GH-02)", Poll::Overlay),
    ] {
        let mut walks = Vec::new();
        let mut answers = Vec::new();
        for round in 0..7 {
            let (took, given) = walk(&path, poll, &request);
            if round > 0 {
                walks.push(took);
                answers.push(given);
            }
        }
        answers.sort_unstable();
        println!(
            "{label}: walk median {:?}, answers given {}",
            median(walks),
            answers[answers.len() / 2]
        );
    }
}

/// GH-03: what the pass reads before the graph may start, and what it used to read too.
#[test]
#[ignore = "needs COGIT_BENCH_DIRTY"]
fn gh03_what_a_pass_reads() {
    let Ok(path) = std::env::var("COGIT_BENCH_DIRTY") else {
        return;
    };
    let state = AppState::new();
    let repo = state.open_repository(Path::new(&path)).unwrap().repo;
    let time = |what: &dyn Fn()| {
        let mut samples = Vec::new();
        for round in 0..12 {
            let started = Instant::now();
            what();
            if round >= 2 {
                samples.push(started.elapsed());
            }
        }
        median(samples)
    };
    let reread = time(&|| {
        state.reread_repository(repo).unwrap();
    });
    let refs = time(&|| {
        state.repo_refs(repo).unwrap();
    });
    let files = time(&|| {
        state.worktree_files(repo, WorktreeView::default()).unwrap();
    });
    let counters = time(&|| {
        state.working_state(repo).unwrap();
    });
    println!(
        "GH-03 reread_repository {reread:?}, repo_refs {refs:?}, worktree_files {files:?}, working_state {counters:?}"
    );
    println!(
        "GH-03 before the graph may start: old {:?} (reread + files + counters), new {refs:?} (refs only)",
        reread + files + counters
    );
}
