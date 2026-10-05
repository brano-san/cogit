//! The same repository reads timed on any root: `C:` against `\\wsl.localhost\…` (M15).
//!
//! `cargo run --release -p app_state --example backend_probe -- <root>…`
//! Per read: the first run, the median, and the answer as JSON — what an agent would send.

use app_state::{AppState, DEFAULT_CHUNK_SIZE};
use std::path::Path;
use std::time::Instant;

const RUNS: usize = 5;

fn out(line: &str) {
    std::io::Write::write_all(&mut std::io::stdout(), format!("{line}\n").as_bytes()).ok();
}

fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

fn ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0
}

fn time<T: serde::Serialize>(what: &str, mut work: impl FnMut() -> T) {
    let mut answer = None;
    let samples: Vec<f64> = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            answer = Some(std::hint::black_box(work()));
            ms(started)
        })
        .collect();
    let first = samples[0];
    let started = Instant::now();
    let json = serde_json::to_vec(&answer).unwrap_or_default();
    let encode = ms(started);
    out(&format!(
        "  {what:<16} first {first:>8.1} ms  median {:>8.1} ms  json {:>9} B in {encode:.2} ms",
        median(samples),
        json.len()
    ));
}

fn main() {
    let state = AppState::new();
    for root in std::env::args().skip(1) {
        let path = Path::new(&root);
        out(&format!(
            "{root} ({:?})",
            app_state::location::RepoLocation::of(path)
        ));
        let repo = match state.open_repository(path) {
            Ok(summary) => summary.repo,
            Err(err) => {
                out(&format!("  cannot open: {err}"));
                continue;
            }
        };
        time("open_repository", || {
            state.open_repository(path).map_err(|e| e.to_string())
        });
        time("repo_status", || {
            state.repo_status(repo).map_err(|e| e.to_string())
        });
        time("worktree_files", || {
            state
                .worktree_files(repo, git_engine::WorktreeView::default())
                .map_err(|e| e.to_string())
        });
        let mut generation = 0;
        time("load_commits", || {
            generation = state.begin_graph();
            state
                .build_graph(
                    repo,
                    &git_engine::CommitQuery::default(),
                    generation,
                    DEFAULT_CHUNK_SIZE,
                    |_| true,
                )
                .map(|skipped| skipped.len())
                .map_err(|e| e.to_string())
        });
        time("graph_window 200", || {
            state.graph_window(repo, generation, 0, 200)
        });
    }
}
