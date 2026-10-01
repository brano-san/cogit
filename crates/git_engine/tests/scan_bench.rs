//! Stage timings of the Remove Worktree scan on a real folder (R-675):
//! `COGIT_SCAN_BENCH=<main> COGIT_SCAN_WORKTREE=<linked> cargo test -p git_engine --release
//! --test scan_bench -- --ignored --nocapture`
#![allow(clippy::unwrap_used, clippy::print_stdout)]
use git_engine::RepoHandle;
use std::time::Instant;

#[test]
#[ignore = "needs COGIT_SCAN_BENCH and COGIT_SCAN_WORKTREE"]
fn stage_timings() {
    let (Ok(main), Ok(wt)) = (
        std::env::var("COGIT_SCAN_BENCH"),
        std::env::var("COGIT_SCAN_WORKTREE"),
    ) else {
        return;
    };
    let handle = RepoHandle::open_exact(std::path::Path::new(&main)).unwrap();
    for round in 0..4 {
        let t = Instant::now();
        let all = handle.worktree_changes(&wt).unwrap();
        let before = t.elapsed();

        let scan = handle.worktree_scan(&wt).unwrap();
        let t = Instant::now();
        let changes = scan.changes().unwrap();
        let (c, ct) = (changes.len(), t.elapsed());
        let t = Instant::now();
        let modules = scan.submodules().unwrap();
        let st = t.elapsed();
        let t = Instant::now();
        let unpushed = scan.unpushed().unwrap();
        let ut = t.elapsed();

        let t = Instant::now();
        std::thread::scope(|s| {
            s.spawn(|| scan.changes().unwrap());
            s.spawn(|| scan.submodules().unwrap());
            s.spawn(|| scan.unpushed().unwrap());
        });
        let par = t.elapsed();
        println!(
            "round {round}: BEFORE changes+submodule walk {before:?} ({} rows) | AFTER changes {ct:?} ({c}), submodules {st:?} ({}), unpushed {ut:?} ({}), all three in parallel {par:?}",
            all.len(),
            modules.changed.len(),
            unpushed.len()
        );
    }
}
