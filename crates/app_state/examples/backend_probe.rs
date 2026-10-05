//! The same repository reads timed on any root: `C:` against `\\wsl.localhost\…` (M15).
//!
//! `cargo run --release -p app_state --example backend_probe -- <root>…`

use app_state::{AppState, DEFAULT_CHUNK_SIZE};
use std::path::Path;
use std::time::Instant;

const RUNS: usize = 5;

fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

fn time<T>(what: &str, mut work: impl FnMut() -> T) {
    let samples: Vec<f64> = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            std::hint::black_box(work());
            started.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    let first = samples[0];
    let line = format!(
        "  {what:<16} first {first:>9.1} ms  median {:>9.1} ms",
        median(samples)
    );
    std::io::Write::write_all(&mut std::io::stdout(), format!("{line}\n").as_bytes()).ok();
}

fn main() {
    let state = AppState::new();
    for root in std::env::args().skip(1) {
        let path = Path::new(&root);
        let header = format!(
            "{root} ({:?})\n",
            app_state::location::RepoLocation::of(path)
        );
        std::io::Write::write_all(&mut std::io::stdout(), header.as_bytes()).ok();
        let repo = match state.open_repository(path) {
            Ok(summary) => summary.repo,
            Err(err) => {
                let line = format!("  cannot open: {err}\n");
                std::io::Write::write_all(&mut std::io::stdout(), line.as_bytes()).ok();
                continue;
            }
        };
        time("open_repository", || {
            state.open_repository(path).map(|s| s.repo)
        });
        time("repo_status", || state.repo_status(repo));
        time("worktree_files", || {
            state.worktree_files(repo, git_engine::WorktreeView::default())
        });
        time("load_commits", || {
            let generation = state.begin_graph();
            state.build_graph(
                repo,
                &git_engine::CommitQuery::default(),
                generation,
                DEFAULT_CHUNK_SIZE,
                |_| true,
            )
        });
    }
}
