//! Timing harnesses for the rework-2 benchmarks (doc/15-benchmark.md), run on request with
//! `COGIT_BENCH_REPO=<large> COGIT_BENCH_REPO_CG=<large with commit-graph> COGIT_BENCH_SCAN=<folder>
//! cargo test -p git_engine --release --test rework_bench -- --ignored --nocapture`.
//! They use only what the old and the new code both have, so the same file measures both.
#![allow(clippy::unwrap_used, clippy::print_stdout)]

use git_engine::discover::{ScanOptions, scan_cancellable};
use git_engine::{RepoHandle, SharedRepo};
use std::path::Path;
use std::time::{Duration, Instant};

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

fn percentile(mut samples: Vec<Duration>, per_cent: usize) -> Duration {
    samples.sort();
    samples[(samples.len() * per_cent / 100).min(samples.len() - 1)]
}

/// GH-05: what a shared handle costs to make.
#[test]
#[ignore = "needs COGIT_BENCH_REPO"]
fn gh05_a_shared_handle() {
    let Ok(path) = std::env::var("COGIT_BENCH_REPO") else {
        return;
    };
    let shared = SharedRepo::open(Path::new(&path)).unwrap();
    let mut made = Vec::new();
    let mut first_read = Vec::new();
    for _ in 0..300 {
        let started = Instant::now();
        let handle = shared.handle().unwrap();
        made.push(started.elapsed());
        let started = Instant::now();
        handle.is_merged_into_head("HEAD").unwrap();
        first_read.push(started.elapsed());
    }
    println!(
        "GH-05 handle(): median {:?}, p95 {:?}; first read {:?}",
        median(made.clone()),
        percentile(made, 95),
        median(first_read)
    );
}

/// GH-06: asking whether a commit is in HEAD, the way a selection change does.
#[test]
#[ignore = "needs COGIT_BENCH_REPO_CG"]
fn gh06_is_merged_into_head() {
    let Ok(path) = std::env::var("COGIT_BENCH_REPO_CG") else {
        return;
    };
    let listing = test_fixtures::git_command_in(Path::new(&path))
        .args(["rev-list", "HEAD"])
        .output()
        .unwrap();
    let oids: Vec<String> = String::from_utf8(listing.stdout)
        .unwrap()
        .lines()
        .step_by(800)
        .map(str::to_owned)
        .collect();
    let handle = RepoHandle::open_exact(Path::new(&path)).unwrap();
    let mut samples = Vec::new();
    for round in 0..5 {
        for oid in &oids {
            let started = Instant::now();
            handle.is_merged_into_head(oid).unwrap();
            if round > 0 {
                samples.push(started.elapsed());
            }
        }
    }
    println!(
        "GH-06 is_merged_into_head over {} commits: median {:?}, p95 {:?}",
        oids.len(),
        median(samples.clone()),
        percentile(samples, 95)
    );
}

/// C-03: how long a short job waits for the global pool while a folder scan runs.
#[test]
#[ignore = "needs COGIT_BENCH_SCAN"]
fn c03_a_scan_beside_short_reads() {
    let Ok(folder) = std::env::var("COGIT_BENCH_SCAN") else {
        return;
    };
    let mut scans = Vec::new();
    let mut waits = Vec::new();
    for _ in 0..5 {
        let scanning = std::sync::atomic::AtomicBool::new(true);
        std::thread::scope(|scope| {
            let probe = scope.spawn(|| {
                let mut waited = Vec::new();
                while scanning.load(std::sync::atomic::Ordering::Relaxed) {
                    let (tx, rx) = std::sync::mpsc::channel();
                    let asked = Instant::now();
                    rayon::spawn(move || {
                        let _ = tx.send(());
                    });
                    rx.recv().unwrap();
                    waited.push(asked.elapsed());
                    std::thread::sleep(Duration::from_millis(1));
                }
                waited
            });
            let started = Instant::now();
            let mut found = 0;
            scan_cancellable(
                Path::new(&folder),
                &ScanOptions::default(),
                || false,
                |_| {
                    found += 1;
                    true
                },
            );
            scans.push(started.elapsed());
            scanning.store(false, std::sync::atomic::Ordering::Relaxed);
            waits.extend(probe.join().unwrap());
        });
    }
    println!(
        "C-03 scan {:?}; short job waited median {:?}, p95 {:?}, max {:?} ({} probes)",
        median(scans),
        median(waits.clone()),
        percentile(waits.clone(), 95),
        waits.iter().max().unwrap(),
        waits.len()
    );
}
