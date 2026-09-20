// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]
// A budget test exists to print a number; `print_stdout` is denied for production (INV-04).
#![allow(clippy::print_stdout)]

//! The budgets of doc/08-diff-engine.md section 9, asserted rather than eyeballed.
//!
//! `test(/budget/)` puts these in the `timing` group, which gives them the machine to
//! themselves and keeps them out of the pre-commit hook (.config/nextest.toml).

use diff_engine::{DiffOptions, FileDiff, MAX_TEXT_BYTES, diff_bytes, diff_text};
use std::time::Instant;

/// Every hundredth line differs: many small hunks cost more to assemble than one big one,
/// so this is the shape worth measuring.
fn pair(lines: usize) -> (String, String) {
    let old: String = (0..lines)
        .map(|i| format!("let value_{i} = compute({i});\n"))
        .collect();
    let new: String = (0..lines)
        .map(|i| {
            if i % 100 == 0 {
                format!("let value_{i} = compute({i} + 1);\n")
            } else {
                format!("let value_{i} = compute({i});\n")
            }
        })
        .collect();
    (old, new)
}

fn hunks(diff: &FileDiff) -> usize {
    match diff {
        FileDiff::Text { hunks, .. } => hunks.len(),
        other => panic!("expected a text diff, got {other:?}"),
    }
}

#[test]
fn a_five_thousand_line_file_diffs_inside_the_budget() {
    let (old, new) = pair(5_000);
    let options = DiffOptions::default();

    // Warm: the first pass pays for the interner growing.
    let _ = diff_text(&old, &new, &options);

    let started = Instant::now();
    let diff = diff_text(&old, &new, &options);
    let elapsed = started.elapsed();

    println!(
        "  5 000 lines {:>6} ms   (budget   50 ms)",
        elapsed.as_millis()
    );
    assert_eq!(hunks(&diff), 50);
    assert!(
        elapsed.as_millis() < 50,
        "5 000 lines took {} ms, budget is 50 ms",
        elapsed.as_millis()
    );
}

#[test]
fn a_hundred_thousand_line_file_diffs_inside_the_budget() {
    let (old, new) = pair(100_000);
    let options = DiffOptions::default();

    let started = Instant::now();
    let diff = diff_text(&old, &new, &options);
    let elapsed = started.elapsed();

    println!(
        "100 000 lines {:>6} ms   (budget 1000 ms)",
        elapsed.as_millis()
    );
    assert_eq!(hunks(&diff), 1_000);
    assert!(
        elapsed.as_millis() < 1_000,
        "100 000 lines took {} ms, budget is 1000 ms",
        elapsed.as_millis()
    );
}

/// The section 9 row for 100 000 lines is unreachable through the production entry point:
/// a file that long is past `MAX_TEXT_BYTES` and is summarised instead of diffed. The
/// budget above measures the engine; this measures what a caller actually gets (R-101).
#[test]
fn a_hundred_thousand_line_file_never_reaches_the_engine_in_production() {
    let (old, new) = pair(100_000);
    assert!(
        old.len() as u64 > MAX_TEXT_BYTES,
        "100 000 lines is {} bytes, cap is {MAX_TEXT_BYTES}",
        old.len()
    );

    let diff = diff_bytes(old.as_bytes(), new.as_bytes(), &DiffOptions::default());

    println!(
        "100 000 lines = {} bytes, cap {MAX_TEXT_BYTES} -> {}",
        old.len(),
        match &diff {
            FileDiff::TooLarge { .. } => "TooLarge",
            _ => "diffed",
        }
    );
    assert!(matches!(diff, FileDiff::TooLarge { .. }), "{diff:?}");
}
