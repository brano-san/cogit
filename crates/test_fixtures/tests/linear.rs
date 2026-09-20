#![allow(clippy::unwrap_used, clippy::expect_used)]

//! `linear(n)` is the fixture three hundred tests are built on. Its shape is pinned here so
//! the way it is produced can change without anybody having to trust that it did not.

use test_fixtures::{AUTHOR_EMAIL, AUTHOR_NAME, BASE_TIMESTAMP, linear};

#[test]
fn it_has_the_requested_number_of_commits() {
    let f = linear(4).unwrap();
    assert_eq!(f.git(&["rev-list", "--count", "HEAD"]).unwrap().trim(), "4");
}

#[test]
fn the_history_is_linear_and_starts_from_one_root() {
    let f = linear(4).unwrap();
    assert_eq!(f.git(&["rev-list", "--merges", "HEAD"]).unwrap().trim(), "");
    assert_eq!(
        f.git(&["rev-list", "--max-parents=0", "HEAD"])
            .unwrap()
            .lines()
            .count(),
        1
    );
}

#[test]
fn each_commit_adds_one_file_and_keeps_the_earlier_ones() {
    let f = linear(3).unwrap();
    let listed = f.git(&["ls-tree", "--name-only", "HEAD"]).unwrap();
    let mut names: Vec<&str> = listed.lines().collect();
    names.sort_unstable();
    assert_eq!(names, ["file0.txt", "file1.txt", "file2.txt"]);
}

#[test]
fn the_files_carry_the_contents_tests_expect() {
    let f = linear(2).unwrap();
    for i in 0..2 {
        let path = f.path().join(format!("file{i}.txt"));
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            format!("content {i}\n")
        );
    }
}

#[test]
fn the_working_tree_is_checked_out_and_clean() {
    let f = linear(3).unwrap();
    assert!(f.path().join("file2.txt").is_file());
    assert_eq!(f.git(&["status", "--porcelain"]).unwrap().trim(), "");
}

#[test]
fn the_messages_are_the_ones_tests_match_on() {
    let f = linear(3).unwrap();
    let log = f.git(&["log", "--format=%s", "--reverse"]).unwrap();
    assert_eq!(
        log.lines().collect::<Vec<_>>(),
        ["commit 0", "commit 1", "commit 2"]
    );
}

#[test]
fn the_branch_is_main() {
    let f = linear(1).unwrap();
    assert_eq!(
        f.git(&["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap()
            .trim(),
        "main"
    );
}

#[test]
fn author_and_committer_are_the_fixture_identity() {
    let f = linear(1).unwrap();
    let who = f.git(&["log", "-1", "--format=%an|%ae|%cn|%ce"]).unwrap();
    assert_eq!(
        who.trim(),
        format!("{AUTHOR_NAME}|{AUTHOR_EMAIL}|{AUTHOR_NAME}|{AUTHOR_EMAIL}")
    );
}

#[test]
fn timestamps_step_forward_from_the_base() {
    let f = linear(3).unwrap();
    let stamps: Vec<i64> = f
        .git(&["log", "--format=%at", "--reverse"])
        .unwrap()
        .lines()
        .map(|line| line.parse().unwrap())
        .collect();

    assert_eq!(stamps[0], BASE_TIMESTAMP);
    assert!(stamps[1] > stamps[0] && stamps[2] > stamps[1], "{stamps:?}");
}

#[test]
fn the_same_n_gives_the_same_commit_ids() {
    assert_eq!(
        linear(3).unwrap().oid("HEAD").unwrap(),
        linear(3).unwrap().oid("HEAD").unwrap()
    );
}

#[test]
fn two_fixtures_do_not_share_a_directory() {
    let a = linear(1).unwrap();
    let b = linear(1).unwrap();
    assert_ne!(a.path(), b.path());
}

#[test]
fn zero_commits_leaves_an_empty_repository() {
    let f = linear(0).unwrap();
    assert!(f.git(&["rev-parse", "HEAD"]).is_err());
}

#[test]
fn a_later_commit_can_still_be_added_by_hand() {
    let f = linear(2).unwrap();
    f.commit_file(9_000, "extra.txt", "added later\n").unwrap();
    assert_eq!(f.git(&["rev-list", "--count", "HEAD"]).unwrap().trim(), "3");
}
