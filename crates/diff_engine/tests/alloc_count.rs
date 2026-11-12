// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]
// A counting `#[global_allocator]` cannot be written without `unsafe`; test-only.
#![allow(unsafe_code)]

//! Heap allocations of the diff and merge paths, counted rather than timed: a shared
//! machine moves timings by a third, a count does not move (doc/15-benchmark.md §10).
//!
//! `cargo test --release -p diff_engine --test alloc_count -- --ignored --nocapture`
//! Prints allocations, bytes, the median time and a hash of the answer, which must not
//! change when only the allocations do.

use diff_engine::{
    DiffOptions, FileInput, Whitespace, diff_many, diff_one, merge3_sides, merge3_with_syntax,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::hash::{Hash as _, Hasher as _};
use std::io::Write as _;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::Instant;

struct Counting;

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

// SAFETY: every call is forwarded to `System` unchanged; only counters are added.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(layout.size() as u64, Relaxed);
        // SAFETY: the caller's contract is passed on as is.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: as above.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(new_size as u64, Relaxed);
        // SAFETY: as above.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

const RUNS: usize = 15;

fn measure<T: std::fmt::Debug>(what: &str, mut work: impl FnMut() -> T) {
    let (allocs, bytes) = (ALLOCS.load(Relaxed), BYTES.load(Relaxed));
    let answer = std::hint::black_box(work());
    let (allocs, bytes) = (ALLOCS.load(Relaxed) - allocs, BYTES.load(Relaxed) - bytes);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    format!("{answer:?}").hash(&mut hasher);
    drop(answer);
    let mut samples: Vec<f64> = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            std::hint::black_box(work());
            started.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    samples.sort_by(f64::total_cmp);
    let line = format!(
        "{what:<28} allocs {allocs:>9}  bytes {bytes:>11}  median {:>8.2} ms  hash {:016x}\n",
        samples[RUNS / 2],
        hasher.finish()
    );
    std::io::stderr().write_all(line.as_bytes()).unwrap();
}

/// Code-like lines: every 10th changed in a word, a block of 40 moved, some deleted and added.
fn source(lines: usize, seed: usize) -> (String, String) {
    let line = |i: usize| format!("    let value_{seed}_{i} = compute(input_{i}, {i}) + offset;\n");
    let old: String = (0..lines).map(line).collect();
    let mut new = String::new();
    for i in 0..lines {
        if (200..240).contains(&i) {
            continue;
        }
        if i == lines - 200 {
            for moved in 200..240 {
                new.push_str(&line(moved));
            }
        }
        match i % 10 {
            3 => new.push_str(&format!(
                "    let value_{seed}_{i} = compute(output_{i}, {i}) - offset;\n"
            )),
            7 if i % 70 == 7 => {}
            9 if i % 90 == 9 => {
                new.push_str(&line(i));
                new.push_str(&format!("    log(\"added after {i}\");\n"));
            }
            _ => new.push_str(&line(i)),
        }
    }
    (old, new)
}

fn merge_sides(lines: usize) -> (String, String, String) {
    let base: String = (0..lines)
        .map(|i| format!("line {i} of the base\n"))
        .collect();
    let side = |every: usize, tag: &str| -> String {
        (0..lines)
            .map(|i| {
                if i % every == 0 {
                    format!("line {i} changed by {tag}\n")
                } else {
                    format!("line {i} of the base\n")
                }
            })
            .collect()
    };
    (base, side(20, "ours"), side(30, "theirs"))
}

#[test]
#[ignore = "a measurement: run by hand with --release --nocapture"]
fn allocations_of_the_diff_and_merge_paths() {
    let options = DiffOptions::default();
    let (old, new) = source(5_000, 0);
    measure("diff_one 5k lines", || {
        diff_one("src/lib.rs", old.as_bytes(), new.as_bytes(), &options)
    });
    let crlf_old = old.replace('\n', "\r\n");
    let crlf_new = new.replace('\n', "\r\n");
    measure("diff_one 5k lines CRLF", || {
        diff_one(
            "src/lib.rs",
            crlf_old.as_bytes(),
            crlf_new.as_bytes(),
            &options,
        )
    });
    let plain = DiffOptions {
        word_diff: false,
        detect_moves: false,
        ..DiffOptions::default()
    };
    measure("diff_one 5k no words/moves", || {
        diff_one("src/lib.txt", old.as_bytes(), new.as_bytes(), &plain)
    });
    let blanks = DiffOptions {
        ignore_whitespace: Whitespace::All,
        ..DiffOptions::default()
    };
    measure("diff_one 5k ignore ws", || {
        diff_one("src/lib.rs", old.as_bytes(), new.as_bytes(), &blanks)
    });

    let files: Vec<FileInput> = (0..60)
        .map(|n| {
            let (old, new) = source(400, n);
            FileInput {
                path: format!("src/file{n}.rs"),
                old: old.into_bytes(),
                new: new.into_bytes(),
                content: diff_engine::Content::Detect,
            }
        })
        .collect();
    measure("diff_many 60 x 400 lines", || {
        diff_many(files.clone(), &options)
    });

    let (base, ours, theirs) = merge_sides(5_000);
    measure("merge3 5k lines", || {
        merge3_with_syntax(&base, &ours, &theirs, None)
    });
    measure("merge3_sides 5k lines", || {
        merge3_sides(&base, &ours, &theirs, None)
    });
}
