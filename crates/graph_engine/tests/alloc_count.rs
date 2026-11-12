//! Heap allocations of the lane layout on a real history, counted on this thread:
//! `COGIT_BENCH_LOG=<file> cargo test -p graph_engine --release --test alloc_count --
//! --ignored --nocapture`, the file being `git log --topo-order --format="%H %P"` of the
//! repository (doc/15-benchmark.md §10). The hash of the rows proves the layout unchanged.
#![allow(unsafe_code)]
#![allow(clippy::unwrap_used, clippy::print_stdout, clippy::cast_precision_loss)]

use graph_engine::{CommitNode, GraphRow, LayoutCursor, finish, push};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hash::{DefaultHasher, Hash, Hasher};

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

fn counted<R>(run: impl FnOnce() -> R) -> (R, u64, u64) {
    COUNT.with(|count| count.set((0, 0)));
    ON.with(|on| on.set(true));
    let out = run();
    ON.with(|on| on.set(false));
    let (n, b) = COUNT.with(Cell::get);
    (out, n, b)
}

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

fn digest(rows: &[GraphRow]) -> u64 {
    let mut hasher = DefaultHasher::new();
    format!("{rows:?}").hash(&mut hasher);
    hasher.finish()
}

#[test]
#[ignore = "needs COGIT_BENCH_LOG"]
fn allocations_of_the_layout() {
    let Ok(path) = std::env::var("COGIT_BENCH_LOG") else {
        return;
    };
    let log = std::fs::read_to_string(path).unwrap();
    for long_links in [0, 40] {
        let commits = nodes(&log);
        let len = commits.len() as u64;
        let chunks: Vec<Vec<CommitNode>> = commits.chunks(200).map(<[_]>::to_vec).collect();
        let (rows, n, b) = counted(|| {
            let mut cursor = LayoutCursor::default().with_long_links(long_links);
            let mut rows = Vec::with_capacity(commits.len());
            for chunk in chunks {
                rows.extend(push(chunk, &mut cursor));
            }
            rows.extend(finish(&mut cursor));
            rows
        });
        println!(
            "long_links {long_links:>2}: {n:>9} allocs {:>8} KiB  {:.2} allocs/row  digest {:016x}",
            b / 1024,
            n as f64 / len as f64,
            digest(&rows)
        );
    }
}
