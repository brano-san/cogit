// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stderr)]

//! What one open repository costs the process, on the benchmark's `large` set (R-351).
//! A measurement, not a check: run it by hand with the path of the set,
//! `COGIT_BENCH_LARGE=…/target/bench/repos/large cargo nextest run -p app_state
//! --test open_memory --run-ignored only --no-capture`.

use app_state::{AppState, DEFAULT_CHUNK_SIZE};
use git_engine::CommitQuery;

/// Resident and private bytes of this process, asked of the system rather than of a
/// crate: `sysinfo` is not a Windows dependency of this crate.
#[derive(Debug)]
struct Probe;

impl Probe {
    fn new() -> Self {
        Self
    }

    #[cfg(windows)]
    fn read(&mut self) -> (u64, u64) {
        let script = format!(
            "$p = Get-Process -Id {}; \"$($p.WorkingSet64) $($p.PrivateMemorySize64)\"",
            std::process::id()
        );
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        let mut parts = text.split_whitespace().map(|n| n.parse::<u64>().unwrap());
        (parts.next().unwrap(), parts.next().unwrap())
    }

    #[cfg(not(windows))]
    fn read(&mut self) -> (u64, u64) {
        let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
        let field = |name: &str| {
            status
                .lines()
                .find(|line| line.starts_with(name))
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|kb| kb.parse::<u64>().ok())
                .map_or(0, |kb| kb * 1024)
        };
        (field("VmRSS:"), field("RssAnon:"))
    }
}

fn mib(bytes: u64, before: u64) -> String {
    let delta = i128::from(bytes) - i128::from(before);
    format!("{:+.1} MiB", delta as f64 / 1_048_576.0)
}

#[test]
#[ignore = "needs the benchmark's large repository in COGIT_BENCH_LARGE"]
fn one_open_large_repository_costs() {
    let Some(path) = std::env::var_os("COGIT_BENCH_LARGE") else {
        eprintln!("COGIT_BENCH_LARGE is not set; nothing measured");
        return;
    };
    let path = std::path::PathBuf::from(path);
    let mut probe = Probe::new();
    let state = AppState::new();
    let (rss0, private0) = probe.read();

    let summary = state.open_repository(&path).unwrap();
    let id = summary.repo;
    drop(summary);
    state.show_repository(Some(id));
    let (rss1, private1) = probe.read();

    let rows = state.overviews();
    assert_eq!(rows.len(), 1);
    let (rss2, private2) = probe.read();

    let generation = state.begin_graph();
    state
        .build_graph(
            id,
            &CommitQuery::default(),
            generation,
            DEFAULT_CHUNK_SIZE,
            |_| true,
        )
        .unwrap();
    let total = state.graph_window(id, generation, 0, 0).unwrap().total;
    let (rss3, private3) = probe.read();

    assert!(state.close_repository(id));
    let (rss4, private4) = probe.read();

    eprintln!(
        "open (watcher, registration): rss {}, private {}",
        mib(rss1, rss0),
        mib(private1, private0)
    );
    eprintln!(
        "tree row: rss {}, private {}",
        mib(rss2, rss1),
        mib(private2, private1)
    );
    eprintln!(
        "graph of {total} rows: rss {}, private {}",
        mib(rss3, rss2),
        mib(private3, private2)
    );
    eprintln!(
        "after close: rss {}, private {} against the start",
        mib(rss4, rss0),
        mib(private4, private0)
    );
}
