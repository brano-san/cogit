//! Timing of the lane layout on a real history: `COGIT_BENCH_LOG=<file> cargo test -p
//! graph_engine --release --test layout_bench -- --ignored --nocapture`, the file being
//! `git log --date-order --format="%H %P"` of the repository (doc/15-benchmark.md).
#![allow(clippy::unwrap_used, clippy::print_stdout)]

use graph_engine::{CommitNode, LayoutCursor, finish, push};
use std::time::{Duration, Instant};

fn nodes(log: &str) -> Vec<CommitNode> {
    log.lines()
        .map(|line| {
            let mut ids = line.split(' ').filter(|id| !id.is_empty());
            CommitNode {
                oid: ids.next().unwrap().to_owned(),
                parents: ids.map(str::to_owned).collect(),
                hidden: Vec::new(),
            }
        })
        .collect()
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

/// A wide history without a repository: each commit takes its parents among the `reach`
/// after it (the log lists children first), so hundreds of lines run side by side and many of them wait for one commit.
#[test]
#[ignore = "timing harness"]
fn layout_of_a_wide_random_history() {
    let (rows, reach) = (30_000usize, 400usize);
    let mut seed = 12_345_u64;
    let mut next = move |bound: usize| {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        usize::try_from(seed >> 33).unwrap() % bound
    };
    let mut log = String::new();
    for row in 0..rows {
        let mut line = format!("c{row}");
        let window = (rows - 1 - row).min(reach);
        for _ in 0..(if next(10) == 0 { 2 } else { 1 }) {
            if window > 0 {
                line += &format!(" c{}", row + 1 + next(window));
            }
        }
        log += &line;
        log.push('\n');
    }
    let mut samples = Vec::new();
    let mut width = 0;
    for _ in 0..7 {
        let commits = nodes(&log);
        let started = Instant::now();
        let mut cursor = LayoutCursor::default();
        let mut laid = push(commits, &mut cursor);
        laid.extend(finish(&mut cursor));
        samples.push(started.elapsed());
        width = laid.iter().map(|row| row.width).max().unwrap_or(0);
    }
    println!(
        "wide random: {:?} for {rows} rows, widest {width}",
        median(samples)
    );
}

#[test]
#[ignore = "needs COGIT_BENCH_LOG"]
fn layout_of_a_history() {
    let Ok(path) = std::env::var("COGIT_BENCH_LOG") else {
        return;
    };
    let log = std::fs::read_to_string(path).unwrap();
    for long_links in [0, 40] {
        let mut samples = Vec::new();
        let mut width = 0;
        for _ in 0..7 {
            let commits = nodes(&log);
            let started = Instant::now();
            let mut cursor = LayoutCursor::default().with_long_links(long_links);
            let mut rows = push(commits, &mut cursor);
            rows.extend(finish(&mut cursor));
            samples.push(started.elapsed());
            width = rows.iter().map(|row| row.width).max().unwrap_or(0);
        }
        println!(
            "long_links {long_links}: {:?} for {} rows, widest {width}",
            median(samples),
            log.lines().count()
        );
    }
}
