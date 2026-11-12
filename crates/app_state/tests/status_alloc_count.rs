//! Heap allocations of the status reads (doc/15-benchmark.md), counted rather than timed
//! because counts do not drift with the load of a shared machine. Run on request with
//! `COGIT_BENCH_DIRTY=<dirty> COGIT_BENCH_REPO=<large> cargo test -p app_state --release
//! --test status_alloc_count -- --ignored --nocapture`.
// A counting `#[global_allocator]` cannot be written without `unsafe`; test-only.
#![allow(unsafe_code, clippy::unwrap_used, clippy::print_stdout)]

use app_state::AppState;
use git_engine::{RepoHandle, WorktreeView};
use std::alloc::{GlobalAlloc, Layout, System};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

struct Counting;

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

// SAFETY: every call forwards to `System` unchanged; the counters are only bookkeeping.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        // SAFETY: the caller's layout contract passes through.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `System` with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        // SAFETY: as in `alloc`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        // SAFETY: as in `dealloc`; `new_size` is the caller's.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// The fewest allocations (and their bytes) of `runs` calls: thread pools and lazy statics
/// only add, so the minimum is the cost of the read itself.
fn count<T>(label: &str, runs: usize, mut read: impl FnMut() -> T) {
    let mut best = (u64::MAX, u64::MAX);
    for _ in 0..runs {
        let (allocs, bytes) = (ALLOCS.load(Ordering::SeqCst), BYTES.load(Ordering::SeqCst));
        std::hint::black_box(read());
        let spent = (
            ALLOCS.load(Ordering::SeqCst) - allocs,
            BYTES.load(Ordering::SeqCst) - bytes,
        );
        best = best.min(spent);
    }
    println!("{label:<36} {:>9} allocs {:>12} bytes", best.0, best.1);
}

fn repo(var: &str) -> Option<String> {
    let path = std::env::var(var).ok();
    if path.is_none() {
        println!("{var} unset, skipped");
    }
    path
}

#[test]
#[ignore = "allocation harness, run on request"]
fn status_reads_allocations() {
    let unchanged = WorktreeView {
        unchanged: true,
        ..WorktreeView::default()
    };
    for (var, name) in [
        ("COGIT_BENCH_DIRTY", "dirty"),
        ("COGIT_BENCH_REPO", "large"),
    ] {
        let Some(path) = repo(var) else { continue };
        let state = AppState::new();
        let id = state.open_repository(Path::new(&path)).unwrap().repo;
        // The open starts a walk on its own thread; join it before counting.
        state.repo_status(id).unwrap();
        state.worktree_files(id, WorktreeView::default()).unwrap();
        count(&format!("{name} repo_status"), 7, || {
            state.repo_status(id).unwrap()
        });
        count(&format!("{name} worktree_files"), 7, || {
            state.worktree_files(id, WorktreeView::default()).unwrap()
        });
        count(&format!("{name} worktree_files unchanged"), 7, || {
            state.worktree_files(id, unchanged).unwrap()
        });
        let handle = RepoHandle::open(Path::new(&path)).unwrap();
        count(&format!("{name} change_counts"), 7, || {
            handle.change_counts().unwrap()
        });
    }
}
