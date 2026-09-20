// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]
// The harness exists to print numbers; `print_stdout` is denied for production (INV-04).
#![allow(clippy::print_stdout)]

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
    let f = stress(50).unwrap();
    assert!(
        f.path().join("data.txt").is_file(),
        "expected a checked-out working tree"
    );
}

#[test]
fn stress_fixtures_do_not_share_state() {
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

#[test]
fn wide_has_the_requested_number_of_files() {
    let f = test_fixtures::wide(300).unwrap();
    let listed = f.git(&["ls-files"]).unwrap();
    assert_eq!(listed.lines().count(), 300);
}

#[test]
fn wide_ends_with_a_commit_touching_one_file() {
    let f = test_fixtures::wide(50).unwrap();
    let changed = f.git(&["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]);
    assert_eq!(changed.unwrap().lines().count(), 1);
}

#[test]
fn wide_is_deterministic() {
    let a = test_fixtures::wide(20).unwrap();
    let b = test_fixtures::wide(20).unwrap();
    assert_eq!(a.oid("HEAD").unwrap(), b.oid("HEAD").unwrap());
}
