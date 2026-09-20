#![allow(clippy::unwrap_used, clippy::expect_used)]

//! `branched()` and `diamond()` are the second and third most used fixtures. Their shape is
//! pinned here so the way they are produced can change without anybody having to trust it.

use test_fixtures::{branched, diamond};

fn subjects(f: &test_fixtures::Fixture, rev: &str) -> Vec<String> {
    f.git(&["log", "--format=%s", "--reverse", rev])
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn branched_leaves_head_on_main() {
    let f = branched().unwrap();
    assert_eq!(
        f.git(&["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap()
            .trim(),
        "main"
    );
}

#[test]
fn branched_has_a_dev_branch_that_left_main_one_commit_back() {
    let f = branched().unwrap();
    let base = f.git(&["merge-base", "main", "dev"]).unwrap();
    assert_eq!(base.trim(), f.oid("main~1").unwrap());
}

#[test]
fn branched_puts_two_commits_on_each_side() {
    let f = branched().unwrap();
    assert_eq!(subjects(&f, "main"), ["commit 0", "commit 1"]);
    assert_eq!(subjects(&f, "dev"), ["commit 0", "commit 2", "commit 3"]);
}

#[test]
fn branched_checks_out_mains_files_and_not_devs() {
    let f = branched().unwrap();
    assert!(f.path().join("base.txt").is_file());
    assert!(f.path().join("main-1.txt").is_file());
    assert!(
        !f.path().join("dev-1.txt").exists(),
        "dev is not checked out"
    );
}

#[test]
fn branched_carries_the_file_contents_tests_read() {
    let f = branched().unwrap();
    let base = std::fs::read_to_string(f.path().join("base.txt")).unwrap();
    assert_eq!(base, "base\n");
    let dev = f.git(&["show", "dev:dev-2.txt"]).unwrap();
    assert_eq!(dev, "dev two\n");
}

#[test]
fn branched_is_clean_and_deterministic() {
    assert_eq!(
        branched()
            .unwrap()
            .git(&["status", "--porcelain"])
            .unwrap()
            .trim(),
        ""
    );
    assert_eq!(
        branched().unwrap().oid("dev").unwrap(),
        branched().unwrap().oid("dev").unwrap()
    );
}

#[test]
fn diamond_ends_on_a_merge_of_two_parents() {
    let f = diamond().unwrap();
    let parents = f.git(&["rev-list", "--parents", "-n1", "HEAD"]).unwrap();
    assert_eq!(parents.split_whitespace().count(), 3, "{parents}");
}

#[test]
fn diamond_names_its_merge() {
    let f = diamond().unwrap();
    assert_eq!(
        f.git(&["log", "-1", "--format=%s"]).unwrap().trim(),
        "merge dev into main"
    );
}

#[test]
fn diamond_puts_the_mainline_first() {
    // The lane algorithm relies on the first parent being the branch that was merged into.
    let f = diamond().unwrap();
    assert_eq!(f.oid("HEAD^1").unwrap(), f.oid("main~1").unwrap());
    assert_eq!(f.oid("HEAD^2").unwrap(), f.oid("dev").unwrap());
}

#[test]
fn diamond_has_four_commits_and_one_root() {
    let f = diamond().unwrap();
    assert_eq!(f.git(&["rev-list", "--count", "HEAD"]).unwrap().trim(), "4");
    assert_eq!(
        f.git(&["rev-list", "--max-parents=0", "HEAD"])
            .unwrap()
            .lines()
            .count(),
        1
    );
}

#[test]
fn diamond_checks_out_both_sides_of_the_merge() {
    let f = diamond().unwrap();
    for name in ["base.txt", "main.txt", "dev.txt"] {
        assert!(f.path().join(name).is_file(), "{name} is missing");
    }
    assert_eq!(f.git(&["status", "--porcelain"]).unwrap().trim(), "");
}

#[test]
fn diamond_is_on_main_and_deterministic() {
    let f = diamond().unwrap();
    assert_eq!(
        f.git(&["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap()
            .trim(),
        "main"
    );
    assert_eq!(
        diamond().unwrap().oid("HEAD").unwrap(),
        f.oid("HEAD").unwrap()
    );
}
