#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{RepoHandle, SplitCut};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn tree_of(f: &test_fixtures::Fixture, rev: &str) -> String {
    f.oid(&format!("{rev}^{{tree}}")).unwrap()
}

fn count(f: &test_fixtures::Fixture) -> u32 {
    f.git(&["rev-list", "--count", "HEAD"])
        .unwrap()
        .trim()
        .parse()
        .unwrap()
}

fn changed_in(f: &test_fixtures::Fixture, rev: &str) -> Vec<String> {
    f.git(&[
        "-c",
        "core.quotepath=false",
        "show",
        "--name-only",
        "--format=",
        "--no-renames",
        rev,
    ])
    .unwrap()
    .lines()
    .filter(|line| !line.is_empty())
    .map(str::to_owned)
    .collect()
}

fn show(f: &test_fixtures::Fixture, rev: &str, path: &str) -> String {
    f.git(&["show", &format!("{rev}:{path}")]).unwrap()
}

fn paths(names: &[&str]) -> SplitCut {
    SplitCut {
        patch: String::new(),
        paths: names.iter().map(|name| (*name).to_owned()).collect(),
    }
}

fn split(
    f: &test_fixtures::Fixture,
    rev: &str,
    cuts: &[SplitCut],
    messages: &[String],
) -> git_engine::Result<()> {
    let mut cuts = cuts.to_vec();
    cuts.push(SplitCut {
        patch: String::new(),
        paths: changed_in(f, rev),
    });
    open(f).split_commit(rev, &cuts, messages).map(drop)
}

fn messages(texts: &[&str]) -> Vec<String> {
    texts.iter().map(|text| (*text).to_owned()).collect()
}

const LINES: &str = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n";

fn mixed() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("lines.txt", LINES).unwrap();
    f.git(&["add", "--", "lines.txt"]).unwrap();
    f.commit_staged(2, "add lines").unwrap();
    f.write_file(
        "lines.txt",
        &LINES
            .replace("\n2\n", "\ntwo\n")
            .replace("19\n", "nineteen\n"),
    )
    .unwrap();
    f.write_file("new.txt", "new\n").unwrap();
    f.write_file("other.txt", "other\n").unwrap();
    f.git(&["add", "--", "lines.txt", "new.txt", "other.txt"])
        .unwrap();
    f.git_at(
        3,
        &[
            "-c",
            "user.name=Original Author",
            "-c",
            "user.email=original@example.com",
            "commit",
            "-m",
            "mixed change\n\nwith a body",
        ],
    )
    .unwrap();
    f
}

const FIRST_HUNK: &str = "diff --git a/lines.txt b/lines.txt\n--- a/lines.txt\n+++ b/lines.txt\n@@ -1,5 +1,5 @@\n 1\n-2\n+two\n 3\n 4\n 5\n";

#[test]
fn head_splits_into_two_commits_with_the_same_final_tree() {
    let f = mixed();
    let target = f.oid("HEAD").unwrap();
    let tree = tree_of(&f, "HEAD");
    let before = count(&f);

    split(
        &f,
        &target,
        &[paths(&["new.txt"])],
        &messages(&["add new", "the rest"]),
    )
    .unwrap();

    assert_eq!(count(&f), before + 1);
    assert_eq!(tree_of(&f, "HEAD"), tree);
    assert_eq!(changed_in(&f, "HEAD~1"), ["new.txt"]);
    assert_eq!(changed_in(&f, "HEAD"), ["lines.txt", "other.txt"]);
    assert_eq!(
        f.git(&["log", "-1", "--format=%B", "HEAD~1"])
            .unwrap()
            .trim(),
        "add new"
    );
    assert_eq!(
        f.git(&["log", "-1", "--format=%B", "HEAD"]).unwrap().trim(),
        "the rest"
    );
}

#[test]
fn every_part_keeps_the_original_author_and_date() {
    let f = mixed();
    let target = f.oid("HEAD").unwrap();
    let original = f
        .git(&["log", "-1", "--format=%an|%ae|%ad", "--date=raw", "HEAD"])
        .unwrap();

    split(
        &f,
        &target,
        &[paths(&["new.txt"])],
        &messages(&["one", "two"]),
    )
    .unwrap();

    for rev in ["HEAD", "HEAD~1"] {
        let author = f
            .git(&["log", "-1", "--format=%an|%ae|%ad", "--date=raw", rev])
            .unwrap();
        assert_eq!(author, original, "{rev}");
    }
}

#[test]
fn head_splits_into_three_commits_by_hunk_and_by_file() {
    let f = mixed();
    let target = f.oid("HEAD").unwrap();
    let tree = tree_of(&f, "HEAD");
    let first = SplitCut {
        patch: FIRST_HUNK.to_owned(),
        paths: Vec::new(),
    };
    let second = SplitCut {
        patch: FIRST_HUNK.to_owned(),
        paths: vec!["new.txt".to_owned()],
    };

    split(
        &f,
        &target,
        &[first, second],
        &messages(&["first hunk", "new file", "rest"]),
    )
    .unwrap();

    assert_eq!(tree_of(&f, "HEAD"), tree);
    assert_eq!(changed_in(&f, "HEAD~2"), ["lines.txt"]);
    let early = show(&f, "HEAD~2", "lines.txt");
    assert!(early.contains("two\n") && early.contains("19\n") && !early.contains("nineteen"));
    assert_eq!(changed_in(&f, "HEAD~1"), ["new.txt"]);
    assert_eq!(changed_in(&f, "HEAD"), ["lines.txt", "other.txt"]);
}

#[test]
fn an_older_commit_splits_and_its_descendants_follow() {
    let f = mixed();
    let target = f.oid("HEAD").unwrap();
    f.commit_file(4, "later.txt", "later\n").unwrap();
    f.write_file("new.txt", "newer\n").unwrap();
    f.git(&["add", "--", "new.txt"]).unwrap();
    f.commit_staged(5, "edit new\n\nbody kept").unwrap();
    let tree = tree_of(&f, "HEAD");
    let before = count(&f);

    split(
        &f,
        &target,
        &[paths(&["other.txt"])],
        &messages(&["other", "rest"]),
    )
    .unwrap();

    assert_eq!(count(&f), before + 1);
    assert_eq!(tree_of(&f, "HEAD"), tree);
    assert_eq!(changed_in(&f, "HEAD~3"), ["other.txt"]);
    assert_eq!(changed_in(&f, "HEAD~2"), ["lines.txt", "new.txt"]);
    assert_eq!(
        f.git(&["log", "-1", "--format=%B", "HEAD"]).unwrap(),
        "edit new\n\nbody kept\n\n"
    );
    assert!(
        !f.git(&["branch", "--contains", &target])
            .unwrap()
            .contains("main")
    );
}

#[test]
fn a_rename_moves_as_one_block() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("old.txt", LINES).unwrap();
    f.git(&["add", "--", "old.txt"]).unwrap();
    f.commit_staged(2, "add old").unwrap();
    f.git(&["mv", "old.txt", "renamed.txt"]).unwrap();
    f.write_file("renamed.txt", &LINES.replace("20\n", "twenty\n"))
        .unwrap();
    f.write_file("extra.txt", "extra\n").unwrap();
    f.git(&["add", "-A"]).unwrap();
    f.commit_staged(3, "rename and add").unwrap();
    let target = f.oid("HEAD").unwrap();
    let tree = tree_of(&f, "HEAD");

    split(
        &f,
        &target,
        &[paths(&["old.txt", "renamed.txt"])],
        &messages(&["rename", "extra"]),
    )
    .unwrap();

    assert_eq!(tree_of(&f, "HEAD"), tree);
    assert_eq!(changed_in(&f, "HEAD~1"), ["old.txt", "renamed.txt"]);
    assert_eq!(changed_in(&f, "HEAD"), ["extra.txt"]);
}

#[test]
fn a_binary_file_moves_whole() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("blob.bin"), [0u8, 1, 2, 255, 0, 10]).unwrap();
    f.write_file("text.txt", "text\n").unwrap();
    f.git(&["add", "--", "blob.bin", "text.txt"]).unwrap();
    f.commit_staged(2, "binary and text").unwrap();
    let target = f.oid("HEAD").unwrap();

    split(
        &f,
        &target,
        &[paths(&["blob.bin"])],
        &messages(&["binary", "text"]),
    )
    .unwrap();

    assert_eq!(changed_in(&f, "HEAD~1"), ["blob.bin"]);
    let blob = f.git(&["rev-parse", "HEAD~1:blob.bin"]).unwrap();
    assert_eq!(
        blob,
        f.git(&["rev-parse", &format!("{target}:blob.bin")])
            .unwrap()
    );
}

#[test]
fn a_deleted_file_is_deleted_in_its_part() {
    let f = mixed();
    f.git(&["rm", "-q", "--", "lines.txt"]).unwrap();
    f.write_file("kept.txt", "kept\n").unwrap();
    f.git(&["add", "--", "kept.txt"]).unwrap();
    f.commit_staged(4, "delete and add").unwrap();
    let target = f.oid("HEAD").unwrap();

    split(
        &f,
        &target,
        &[paths(&["lines.txt"])],
        &messages(&["delete", "add"]),
    )
    .unwrap();

    assert_eq!(changed_in(&f, "HEAD~1"), ["lines.txt"]);
    assert!(f.git(&["show", "HEAD~1:lines.txt"]).is_err());
}

#[test]
fn uncommitted_work_survives_splitting_head() {
    let f = mixed();
    let target = f.oid("HEAD").unwrap();
    f.write_file("other.txt", "staged edit\n").unwrap();
    f.git(&["add", "--", "other.txt"]).unwrap();
    f.write_file("new.txt", "unstaged edit\n").unwrap();

    split(
        &f,
        &target,
        &[paths(&["new.txt"])],
        &messages(&["one", "two"]),
    )
    .unwrap();

    assert_eq!(
        f.git(&["diff", "--cached", "--name-only"]).unwrap().trim(),
        "other.txt"
    );
    assert_eq!(f.git(&["diff", "--name-only"]).unwrap().trim(), "new.txt");
    assert_eq!(
        std::fs::read_to_string(f.path().join("new.txt")).unwrap(),
        "unstaged edit\n"
    );
}

#[test]
fn a_merge_after_the_split_is_refused() {
    let f = mixed();
    let target = f.oid("HEAD").unwrap();
    f.git(&["switch", "-q", "-c", "side"]).unwrap();
    f.commit_file(4, "side.txt", "side\n").unwrap();
    f.git(&["switch", "-q", "-"]).unwrap();
    f.commit_file(5, "main.txt", "main\n").unwrap();
    f.merge(6, &["side"], "merge side").unwrap();

    let why = refused(&f, &target, &[paths(&["new.txt"])], &["one", "two"]);
    assert!(why.contains("would flatten the merge"), "{why}");
}

#[test]
fn undo_by_moving_the_branch_back_restores_the_old_tip() {
    let f = mixed();
    let target = f.oid("HEAD").unwrap();

    let handle = open(&f);
    split(
        &f,
        &target,
        &[paths(&["new.txt"])],
        &messages(&["one", "two"]),
    )
    .unwrap();
    assert_eq!(f.oid("ORIG_HEAD").unwrap(), target);
    handle.move_branch_back("main", &target).unwrap();

    assert_eq!(f.oid("HEAD").unwrap(), target);
}

fn refused(f: &test_fixtures::Fixture, rev: &str, cuts: &[SplitCut], texts: &[&str]) -> String {
    let head = f.oid("HEAD").unwrap();
    let err = split(f, rev, cuts, &messages(texts)).unwrap_err();
    assert_eq!(
        f.oid("HEAD").unwrap(),
        head,
        "the branch moved on a refusal"
    );
    err.to_string()
}

#[test]
fn a_merge_commit_is_refused() {
    let f = test_fixtures::diamond().unwrap();
    let merge = f.oid("HEAD").unwrap();
    let why = refused(&f, &merge, &[paths(&["a.txt"])], &["a", "b"]);
    assert!(why.contains("merge commit"), "{why}");
}

#[test]
fn a_commit_off_the_first_parent_history_is_refused() {
    let f = mixed();
    f.git(&["switch", "-q", "-c", "side"]).unwrap();
    let side = f.commit_file(4, "side.txt", "side\n").unwrap();
    f.write_file("side2.txt", "x\n").unwrap();
    f.git(&["add", "--", "side2.txt"]).unwrap();
    f.commit_staged(5, "side two").unwrap();
    f.git(&["switch", "-q", "-"]).unwrap();
    f.commit_file(6, "main.txt", "main\n").unwrap();
    f.merge(7, &["side"], "merge side").unwrap();

    let why = refused(&f, &side, &[paths(&["side.txt"])], &["a", "b"]);
    assert!(why.contains("first-parent"), "{why}");
}

#[test]
fn a_commit_of_another_branch_is_refused() {
    let f = mixed();
    f.git(&["switch", "-q", "-c", "other"]).unwrap();
    let elsewhere = f.commit_file(4, "x.txt", "x\n").unwrap();
    f.git(&["switch", "-q", "-"]).unwrap();
    let why = refused(&f, &elsewhere, &[paths(&["x.txt"])], &["a", "b"]);
    assert!(why.contains("first-parent"), "{why}");
}

#[test]
fn a_part_without_blocks_is_refused() {
    let f = mixed();
    let target = f.oid("HEAD").unwrap();
    let why = refused(&f, &target, &[SplitCut::default()], &["a", "b"]);
    assert!(why.contains("commit 1"), "{why}");
}

#[test]
fn a_last_part_left_empty_is_refused() {
    let f = mixed();
    let target = f.oid("HEAD").unwrap();
    let all = paths(&["lines.txt", "new.txt", "other.txt"]);
    let why = refused(&f, &target, &[all], &["a", "b"]);
    assert!(why.contains("commit 2"), "{why}");
}

#[test]
fn a_part_without_a_message_is_refused() {
    let f = mixed();
    let target = f.oid("HEAD").unwrap();
    let why = refused(&f, &target, &[paths(&["new.txt"])], &["a", "  "]);
    assert!(why.contains("message"), "{why}");
}

#[test]
fn the_root_commit_can_be_split() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("another.txt", "another\n").unwrap();
    f.git(&["add", "--", "another.txt"]).unwrap();
    f.git_at(2, &["commit", "--amend", "--no-edit"]).unwrap();
    let root = f.oid("HEAD").unwrap();
    let tree = tree_of(&f, "HEAD");

    split(
        &f,
        &root,
        &[paths(&["another.txt"])],
        &messages(&["one", "two"]),
    )
    .unwrap();

    assert_eq!(count(&f), 2);
    assert_eq!(changed_in(&f, "HEAD^"), ["another.txt"]);
    assert_eq!(tree_of(&f, "HEAD"), tree);
}

#[test]
fn a_detached_head_is_refused() {
    let f = mixed();
    let target = f.oid("HEAD").unwrap();
    f.git(&["switch", "-q", "--detach"]).unwrap();
    let why = refused(&f, &target, &[paths(&["new.txt"])], &["a", "b"]);
    assert!(why.contains("branch"), "{why}");
}

#[test]
fn a_patch_that_does_not_apply_moves_nothing() {
    let f = mixed();
    let target = f.oid("HEAD").unwrap();
    let wrong = SplitCut {
        patch: FIRST_HUNK.replace("-2\n", "-zwei\n"),
        paths: Vec::new(),
    };
    refused(&f, &target, &[wrong], &["a", "b"]);
}

#[test]
fn a_final_tree_missing_a_block_is_refused() {
    let f = mixed();
    let target = f.oid("HEAD").unwrap();
    let err = open(&f)
        .split_commit(
            &target,
            &[paths(&["new.txt"]), paths(&["new.txt", "other.txt"])],
            &messages(&["first", "second"]),
        )
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("does not reproduce the original commit tree")
    );
    assert_eq!(f.oid("HEAD").unwrap(), target);
}

#[test]
fn a_file_cannot_replace_a_directory_before_its_children_are_removed() {
    let f = test_fixtures::linear(1).unwrap();
    f.commit_file(2, "folder/old.txt", "inside the folder\n")
        .unwrap();
    f.git(&["rm", "--", "folder/old.txt"]).unwrap();
    f.write_file("folder", "replacement\n").unwrap();
    f.write_file("another.txt", "another\n").unwrap();
    f.git(&["add", "--", "folder", "another.txt"]).unwrap();
    f.commit_staged(3, "replace a folder").unwrap();
    let target = f.oid("HEAD").unwrap();
    refused(&f, &target, &[paths(&["folder"])], &["one", "two"]);

    split(
        &f,
        &target,
        &[paths(&["folder/old.txt"])],
        &messages(&["remove folder", "add replacement"]),
    )
    .unwrap();
    assert_eq!(changed_in(&f, "HEAD^"), ["folder/old.txt"]);
    assert_eq!(tree_of(&f, "HEAD"), tree_of(&f, &target));
}

#[test]
fn an_older_split_requires_a_clean_worktree_before_replaying() {
    let f = mixed();
    let target = f.oid("HEAD").unwrap();
    f.commit_file(4, "later.txt", "later\n").unwrap();
    f.write_file("later.txt", "unstaged\n").unwrap();
    let why = refused(&f, &target, &[paths(&["new.txt"])], &["one", "two"]);
    assert!(why.contains("a rebase needs a clean working tree"), "{why}");
}

#[test]
fn signing_is_required_when_enabled_in_the_repository() {
    let f = mixed();
    f.git(&["config", "commit.gpgSign", "true"]).unwrap();
    f.git(&[
        "config",
        "gpg.program",
        "cogit-test-signing-program-does-not-exist",
    ])
    .unwrap();
    let target = f.oid("HEAD").unwrap();
    let why = refused(&f, &target, &[paths(&["new.txt"])], &["one", "two"]);
    assert!(why.contains(" -S ") && why.contains("failed"), "{why}");
}
