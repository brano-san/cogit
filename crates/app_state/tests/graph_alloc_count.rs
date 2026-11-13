//! Heap allocations of the commit-graph path, counted on the calling thread only (the text
//! reader runs on its own): `COGIT_BENCH_REPO=<large> cargo test -p app_state --release --test
//! graph_alloc_count -- --ignored --nocapture` (doc/15-benchmark.md §10).
#![allow(unsafe_code)]
#![allow(clippy::unwrap_used, clippy::print_stdout)]

use app_state::{AppState, DEFAULT_CHUNK_SIZE};
use git_engine::CommitQuery;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::time::{Duration, Instant};

struct Counting;

thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<(u64, u64)> = const { Cell::new((0, 0)) };
}

fn note(bytes: usize) {
    let _ = ON.try_with(|on| {
        if on.get() {
            let _ = COUNT.try_with(|count| {
                let (n, b) = count.get();
                count.set((n + 1, b + bytes as u64));
            });
        }
    });
}

// SAFETY: every call goes straight to `System`; the counter only reads thread-locals.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        // SAFETY: the caller's contract, passed on.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        // SAFETY: the caller's contract, passed on.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller's contract, passed on.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note(new_size);
        // SAFETY: the caller's contract, passed on.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Allocations (with reallocations) and bytes asked for while `run` runs on this thread.
fn counted<R>(run: impl FnOnce() -> R) -> (R, u64, u64) {
    COUNT.with(|count| count.set((0, 0)));
    ON.with(|on| on.set(true));
    let started = Instant::now();
    let out = run();
    let elapsed = started.elapsed();
    ON.with(|on| on.set(false));
    let (n, b) = COUNT.with(Cell::get);
    println!("    ({} ms)", elapsed.as_millis());
    (out, n, b)
}

fn line(label: &str, n: u64, b: u64, per: u64) {
    println!(
        "{label:<40} {n:>10} allocs {:>10} KiB   {:>6.2} allocs/{}",
        b / 1024,
        n as f64 / per.max(1) as f64,
        if per > 1 { "row" } else { "call" }
    );
}

fn build(state: &AppState, repo: app_state::RepoId, query: &CommitQuery) -> (u32, u32) {
    let generation = state.begin_graph();
    let mut rows = 0;
    state
        .build_graph(repo, query, generation, DEFAULT_CHUNK_SIZE, |p| {
            rows = p.total;
            true
        })
        .unwrap();
    (generation, rows)
}

#[test]
#[ignore = "needs COGIT_BENCH_REPO"]
fn allocations_of_the_graph_path() {
    let Ok(path) = std::env::var("COGIT_BENCH_REPO") else {
        return;
    };
    let state = AppState::new();
    let repo = state.open_repository(Path::new(&path)).unwrap().repo;
    // Warm: the first walk of a process pays for gix's lazily built state.
    build(
        &state,
        repo,
        &CommitQuery {
            long_link_rows: Some(39),
            ..CommitQuery::default()
        },
    );

    let query = CommitQuery {
        long_link_rows: Some(40),
        ..CommitQuery::default()
    };
    let cold = AppState::new();
    let cold_repo = cold.open_repository(Path::new(&path)).unwrap().repo;
    let ((generation, rows), n, b) = counted(|| build(&cold, cold_repo, &query));
    line("cold load (long links 40)", n, b, u64::from(rows));

    let (bytes, n, b) = counted(|| cold.graph_window_wire(cold_repo, generation, 25_000, 200));
    assert!(!bytes.is_empty());
    line("window 200 rows, texts unread", n, b, 200);

    let until = Instant::now() + Duration::from_secs(30);
    while cold.graph_texts_read(cold_repo) < rows as usize && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    let (_, n, b) = counted(|| cold.graph_window_wire(cold_repo, generation, 30_000, 200));
    line("window 200 rows, texts read", n, b, 200);
    println!(
        "wire digest {:016x}",
        digest(&cold, cold_repo, generation, rows)
    );

    let ticked = CommitQuery {
        visible_refs: Some(vec!["HEAD".to_owned(), "refs/heads/feature/000".to_owned()]),
        ..query.clone()
    };
    let ((tick_gen, again), n, b) = counted(|| build(&cold, cold_repo, &ticked));
    line("reuse walk (tick)", n, b, u64::from(again));
    println!(
        "wire digest {:016x}",
        digest(&cold, cold_repo, tick_gen, again)
    );

    let plain = AppState::new();
    let plain_repo = plain.open_repository(Path::new(&path)).unwrap().repo;
    let ((plain_gen, rows), n, b) = counted(|| build(&plain, plain_repo, &CommitQuery::default()));
    line("cold load (no long links)", n, b, u64::from(rows));
    println!(
        "wire digest {:016x}",
        digest(&plain, plain_repo, plain_gen, rows)
    );

    // A flat list: every commit read and matched, and every parent asked whether it is shown.
    let filtered = AppState::new();
    let filtered_repo = filtered.open_repository(Path::new(&path)).unwrap().repo;
    let by_author = CommitQuery {
        author: Some("BENCH".to_owned()),
        ..CommitQuery::default()
    };
    let ((filtered_gen, rows), n, b) = counted(|| build(&filtered, filtered_repo, &by_author));
    line("filtered by author (every commit)", n, b, u64::from(rows));
    println!(
        "wire digest {:016x}",
        digest(&filtered, filtered_repo, filtered_gen, rows)
    );
}

/// The wire bytes of the whole graph: equal before and after means the layout is too.
fn digest(state: &AppState, repo: app_state::RepoId, generation: u32, rows: u32) -> u64 {
    let mut hasher = DefaultHasher::new();
    for start in (0..rows).step_by(200) {
        state
            .graph_window_wire(repo, generation, start, 200)
            .hash(&mut hasher);
    }
    hasher.finish()
}
