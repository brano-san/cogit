// Integration tests are test code by definition, but `allow-unwrap-in-tests` in
// clippy.toml only covers the bodies of `#[test]` functions — helpers beside them are
// still linted. Panicking is how a test reports failure, so allow it for the file.
#![allow(clippy::unwrap_used, clippy::expect_used)]
// The timing harness exists to print numbers; `print_stdout` is denied workspace-wide
// so that production code logs through `tracing` (INV-04), which does not apply here.
#![allow(clippy::print_stdout)]

//! The large fixture used for performance work.

use test_fixtures::stress;

#[test]
fn stress_has_the_requested_number_of_commits() {
    let f = stress(500).unwrap();
    let count = f.git(&["rev-list", "--count", "HEAD"]).unwrap();
    assert_eq!(count.trim(), "500");
}

#[test]
fn stress_history_is_linear() {
    let f = stress(200).unwrap();
    let merges = f.git(&["rev-list", "--merges", "--count", "HEAD"]).unwrap();
    assert_eq!(merges.trim(), "0");
}

#[test]
fn stress_checks_out_a_working_tree() {
    // Benchmarks for `status` and diffing need real files on disk, not just objects.
    let f = stress(50).unwrap();
    assert!(
        f.path().join("data.txt").is_file(),
        "expected a checked-out working tree"
    );
}

#[test]
fn stress_fixtures_do_not_share_state() {
    // The plan originally called for one cached instance reused by every test, which
    // would have made tests depend on each other's mutations. Each call is its own.
    let a = stress(20).unwrap();
    let b = stress(20).unwrap();
    assert_ne!(a.path(), b.path());

    a.commit_file(9_000, "only-in-a.txt", "local change\n")
        .unwrap();
    assert_eq!(
        a.git(&["rev-list", "--count", "HEAD"]).unwrap().trim(),
        "21"
    );
    assert_eq!(
        b.git(&["rev-list", "--count", "HEAD"]).unwrap().trim(),
        "20"
    );
}

#[test]
fn stress_history_is_deterministic() {
    let a = stress(30).unwrap().oid("HEAD").unwrap();
    let b = stress(30).unwrap().oid("HEAD").unwrap();
    assert_eq!(a, b, "snapshot tests depend on stable OIDs");
}

/// Measurement harness, not an assertion.
///
/// Budgets in `doc/modules/M9-fixtures.md` are set from these numbers rather than
/// guessed. Run on demand:
///
/// ```sh
/// cargo test -p test_fixtures --test stress -- --ignored --nocapture
/// ```
#[test]
#[ignore = "measurement, not a check"]
fn report_fixture_timings() {
    use std::time::Instant;

    let measure = |label: &str, f: &dyn Fn()| {
        let start = Instant::now();
        f();
        println!(
            "{label:<28} {:>8.0} ms",
            start.elapsed().as_secs_f64() * 1000.0
        );
    };

    measure("linear(5)", &|| {
        test_fixtures::linear(5).unwrap();
    });
    measure("diamond()", &|| {
        test_fixtures::diamond().unwrap();
    });
    measure("with_submodule()", &|| {
        test_fixtures::with_submodule().unwrap();
    });
    measure("stress(1_000)", &|| {
        stress(1_000).unwrap();
    });
    measure("stress(10_000)", &|| {
        stress(10_000).unwrap();
    });
}
