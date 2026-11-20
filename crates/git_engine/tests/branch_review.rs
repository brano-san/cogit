#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{FileStatus, GitError, RepoHandle};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn paths(files: &[git_engine::FileEntry]) -> Vec<(&str, FileStatus)> {
    files
        .iter()
        .map(|file| (file.path.as_str(), file.status))
        .collect()
}

#[test]
fn the_base_defaults_to_main() {
    let f = test_fixtures::branched().unwrap();
    assert_eq!(
        open(&f).review_base("dev").unwrap().as_deref(),
        Some("main")
    );
}

#[test]
fn the_base_is_never_the_branch_itself() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["branch", "-m", "main", "master"]).unwrap();
    f.git(&["branch", "main", "dev"]).unwrap();
    assert_eq!(
        open(&f).review_base("main").unwrap().as_deref(),
        Some("master")
    );
}

#[test]
fn the_remote_default_branch_comes_first() {
    let f = test_fixtures::branched().unwrap();
    let main = f.oid("main").unwrap();
    f.git(&["update-ref", "refs/remotes/origin/trunk", &main])
        .unwrap();
    f.git(&[
        "symbolic-ref",
        "refs/remotes/origin/HEAD",
        "refs/remotes/origin/trunk",
    ])
    .unwrap();
    assert_eq!(
        open(&f).review_base("dev").unwrap().as_deref(),
        Some("origin/trunk")
    );
}

#[test]
fn no_main_line_means_no_default() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["branch", "-m", "main", "trunk"]).unwrap();
    assert_eq!(open(&f).review_base("dev").unwrap(), None);
}

#[test]
fn the_summary_counts_both_sides_from_the_merge_base() {
    let f = test_fixtures::branched().unwrap();
    let review = open(&f).branch_review("main", "dev").unwrap();
    let expected = f.oid("main~1").unwrap();
    assert_eq!(review.merge_base, expected);
    assert_eq!(review.merge_bases, vec![expected]);
    assert_eq!((review.ahead, review.behind), (2, 1));
    let summaries: Vec<&str> = review.commits.iter().map(|c| c.summary.as_str()).collect();
    assert_eq!(summaries, ["commit 3", "commit 2"]);
    assert_eq!(review.branch_oid, f.oid("dev").unwrap());
    assert!(!review.is_head);
}

#[test]
fn files_are_the_three_dot_diff_without_the_base_side_changes() {
    let f = test_fixtures::branched().unwrap();
    let handle = open(&f);
    let review = handle.branch_review("main", "dev").unwrap();
    let files = handle
        .review_files(&review.merge_base, "dev", false)
        .unwrap();
    assert_eq!(
        paths(&files),
        [
            ("dev-1.txt", FileStatus::Added),
            ("dev-2.txt", FileStatus::Added)
        ]
    );
}

#[test]
fn a_rename_on_the_branch_is_listed_as_one() {
    let f = test_fixtures::Fixture::init().unwrap();
    let body: String = (0..40).map(|i| format!("line {i}\n")).collect();
    f.commit_file(0, "old.txt", &body).unwrap();
    f.git(&["switch", "-c", "topic"]).unwrap();
    f.git(&["mv", "old.txt", "new.txt"]).unwrap();
    f.commit_staged(1, "rename").unwrap();
    let handle = open(&f);
    let review = handle.branch_review("main", "topic").unwrap();
    let files = handle
        .review_files(&review.merge_base, "topic", false)
        .unwrap();
    assert_eq!(paths(&files), [("new.txt", FileStatus::Renamed)]);
    assert_eq!(files[0].old_path.as_deref(), Some("old.txt"));
}

#[test]
fn a_branch_already_merged_has_nothing_to_review() {
    let f = test_fixtures::branched().unwrap();
    f.merge(10, &["dev"], "merge dev").unwrap();
    let handle = open(&f);
    let review = handle.branch_review("main", "dev").unwrap();
    assert_eq!(review.ahead, 0);
    assert!(review.commits.is_empty());
    assert_eq!(review.merge_base, f.oid("dev").unwrap());
    assert!(
        handle
            .review_files(&review.merge_base, "dev", false)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn unrelated_histories_are_refused_with_the_reason() {
    let f = test_fixtures::two_roots().unwrap();
    let err = open(&f).branch_review("main", "orphan").unwrap_err();
    match err {
        GitError::InvalidState(text) => assert!(text.contains("unrelated histories"), "{text}"),
        other => panic!("expected InvalidState, got {other:?}"),
    }
}

#[test]
fn a_criss_cross_picks_what_git_merge_base_picks() {
    let f = test_fixtures::Fixture::init().unwrap();
    f.commit_file(0, "a.txt", "root\n").unwrap();
    f.git(&["branch", "topic"]).unwrap();
    f.commit_file(1, "main.txt", "main\n").unwrap();
    f.git(&["switch", "topic"]).unwrap();
    f.commit_file(2, "topic.txt", "topic\n").unwrap();
    let main_tip = f.oid("main").unwrap();
    let topic_tip = f.oid("topic").unwrap();
    f.merge(3, &[&main_tip], "topic takes main").unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.merge(4, &[&topic_tip], "main takes topic").unwrap();
    f.commit_file(5, "main-2.txt", "main again\n").unwrap();
    f.git(&["switch", "topic"]).unwrap();
    f.commit_file(6, "topic-2.txt", "topic again\n").unwrap();
    f.git(&["switch", "main"]).unwrap();

    let review = open(&f).branch_review("main", "topic").unwrap();
    let git_pick = f.git(&["merge-base", "main", "topic"]).unwrap();
    let mut all: Vec<String> = f
        .git(&["merge-base", "--all", "main", "topic"])
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    all.sort();
    let mut found = review.merge_bases.clone();
    found.sort();
    assert_eq!(review.merge_base, git_pick.trim());
    assert_eq!(found, all);
    assert_eq!(found.len(), 2);
}

#[test]
fn the_working_tree_joins_the_review_of_the_checked_out_branch() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["switch", "dev"]).unwrap();
    f.write_file("dev-1.txt", "dev one, edited\n").unwrap();
    f.write_file("base.txt", "base\n").unwrap();
    std::fs::remove_file(f.path().join("dev-2.txt")).unwrap();
    f.write_file("staged.txt", "new\n").unwrap();
    f.git(&["add", "staged.txt"]).unwrap();
    f.write_file("untracked.txt", "left out\n").unwrap();
    let handle = open(&f);
    let review = handle.branch_review("main", "dev").unwrap();
    assert!(review.is_head);
    let files = handle
        .review_files(&review.merge_base, "dev", true)
        .unwrap();
    assert_eq!(
        paths(&files),
        [
            ("dev-1.txt", FileStatus::Added),
            ("staged.txt", FileStatus::Added)
        ]
    );
}

#[test]
fn a_change_undone_on_disk_leaves_the_review() {
    let f = test_fixtures::branched().unwrap();
    f.git(&["switch", "dev"]).unwrap();
    std::fs::remove_file(f.path().join("dev-1.txt")).unwrap();
    f.write_file("base.txt", "changed\n").unwrap();
    let handle = open(&f);
    let review = handle.branch_review("main", "dev").unwrap();
    let files = handle
        .review_files(&review.merge_base, "dev", true)
        .unwrap();
    assert_eq!(
        paths(&files),
        [
            ("base.txt", FileStatus::Modified),
            ("dev-2.txt", FileStatus::Added)
        ]
    );
}

#[test]
fn only_the_checked_out_branch_has_a_working_tree_to_include() {
    let f = test_fixtures::branched().unwrap();
    let handle = open(&f);
    let review = handle.branch_review("main", "dev").unwrap();
    assert!(matches!(
        handle.review_files(&review.merge_base, "dev", true),
        Err(GitError::InvalidState(_))
    ));
}

#[test]
fn a_branch_touching_thousands_of_files_lists_them_all() {
    let f = test_fixtures::Fixture::init().unwrap();
    f.commit_file(0, "root.txt", "root\n").unwrap();
    f.git(&["switch", "-c", "wide"]).unwrap();
    for i in 0..1500 {
        f.write_file(&format!("dir{}/file{i}.txt", i % 30), &format!("{i}\n"))
            .unwrap();
    }
    f.git(&["add", "."]).unwrap();
    f.commit_staged(1, "many files").unwrap();
    let handle = open(&f);
    let review = handle.branch_review("main", "wide").unwrap();
    let files = handle
        .review_files(&review.merge_base, "wide", false)
        .unwrap();
    assert_eq!(files.len(), 1500);
    assert!(files.windows(2).all(|pair| pair[0].path < pair[1].path));
}
