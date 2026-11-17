#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::RepoHandle;

const TEN: &str = "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\n";

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn committed(files: &[&str]) -> test_fixtures::Fixture {
    let f = test_fixtures::Fixture::init().unwrap();
    for name in files {
        f.write_file(name, TEN).unwrap();
        f.git(&["add", name]).unwrap();
    }
    f.commit_staged(1, "ten lines").unwrap();
    f
}

fn read(f: &test_fixtures::Fixture, name: &str) -> String {
    std::fs::read_to_string(f.path().join(name)).unwrap()
}

fn show(f: &test_fixtures::Fixture, rev: &str) -> String {
    f.git(&["show", rev]).unwrap()
}

fn second_changed(name: &str) -> String {
    format!("--- a/{name}\n+++ b/{name}\n@@ -1,3 +1,3 @@\n one\n-two\n+TWO\n three\n")
}

#[test]
fn one_block_of_a_file_goes_and_the_other_stays() {
    let f = committed(&["a.txt"]);
    f.write_file("a.txt", &TEN.replace("two", "TWO").replace("nine", "NINE"))
        .unwrap();
    let patch = second_changed("a.txt");

    let stash = open(&f).stash_hunks(&patch, &patch, "first block").unwrap();

    assert_eq!(read(&f, "a.txt"), TEN.replace("nine", "NINE"));
    assert_eq!(
        show(&f, &format!("{stash}:a.txt")),
        TEN.replace("two", "TWO")
    );
    let listed = open(&f).stashes().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].oid, stash);
    assert!(listed[0].message.contains("first block"), "{listed:?}");
    assert_eq!(
        f.git(&["diff", "--cached", "--name-only"]).unwrap().trim(),
        ""
    );
}

#[test]
fn only_the_chosen_lines_of_a_block_go() {
    let f = committed(&["a.txt"]);
    f.write_file("a.txt", &TEN.replace("one\n", "one\nfirst\nsecond\n"))
        .unwrap();
    let stage = "--- a/a.txt\n+++ b/a.txt\n@@ -1,1 +1,2 @@\n one\n+first\n";
    let discard = "--- a/a.txt\n+++ b/a.txt\n@@ -1,2 +1,3 @@\n one\n+first\n second\n";

    let stash = open(&f).stash_hunks(stage, discard, "").unwrap();

    assert_eq!(read(&f, "a.txt"), TEN.replace("one\n", "one\nsecond\n"));
    assert_eq!(
        show(&f, &format!("{stash}:a.txt")),
        TEN.replace("one\n", "one\nfirst\n")
    );
    assert!(
        open(&f).stashes().unwrap()[0]
            .message
            .starts_with("WIP on "),
        "{:?}",
        open(&f).stashes().unwrap()
    );
}

#[test]
fn blocks_of_several_files_go_into_one_stash() {
    let f = committed(&["a.txt", "b.txt", "c.txt"]);
    for name in ["a.txt", "b.txt", "c.txt"] {
        f.write_file(name, &TEN.replace("two", "TWO")).unwrap();
    }
    let patch = second_changed("a.txt") + &second_changed("b.txt");

    let stash = open(&f).stash_hunks(&patch, &patch, "two files").unwrap();

    assert_eq!(read(&f, "a.txt"), TEN);
    assert_eq!(read(&f, "b.txt"), TEN);
    assert_eq!(read(&f, "c.txt"), TEN.replace("two", "TWO"));
    let stashed = f
        .git(&["diff", "--name-only", &format!("{stash}^1"), &stash])
        .unwrap();
    assert_eq!(stashed.lines().collect::<Vec<_>>(), ["a.txt", "b.txt"]);
}

#[test]
fn staged_changes_stay_staged_and_out_of_the_stash() {
    let f = committed(&["a.txt", "b.txt"]);
    f.write_file("a.txt", &TEN.replace("nine", "NINE")).unwrap();
    f.write_file("b.txt", &TEN.replace("five", "FIVE")).unwrap();
    f.git(&["add", "a.txt", "b.txt"]).unwrap();
    f.write_file("a.txt", &TEN.replace("nine", "NINE").replace("two", "TWO"))
        .unwrap();
    let staged_before = f.git(&["diff", "--cached"]).unwrap();
    let patch = second_changed("a.txt");

    let stash = open(&f).stash_hunks(&patch, &patch, "").unwrap();

    assert_eq!(f.git(&["diff", "--cached"]).unwrap(), staged_before);
    assert_eq!(read(&f, "a.txt"), TEN.replace("nine", "NINE"));
    assert_eq!(f.git(&["diff", "--name-only"]).unwrap().trim(), "");
    assert_eq!(
        show(&f, &format!("{stash}:a.txt")),
        TEN.replace("two", "TWO")
    );
    assert_eq!(
        f.git(&["rev-parse", &format!("{stash}^2^{{tree}}")])
            .unwrap(),
        f.git(&["rev-parse", "HEAD^{tree}"]).unwrap()
    );
    f.git(&["stash", "pop"]).unwrap();
    assert_eq!(
        read(&f, "a.txt"),
        TEN.replace("nine", "NINE").replace("two", "TWO")
    );
}

#[test]
fn applying_the_stash_brings_back_only_the_stashed_lines() {
    let f = committed(&["a.txt"]);
    f.write_file("a.txt", &TEN.replace("two", "TWO").replace("nine", "NINE"))
        .unwrap();
    let patch = second_changed("a.txt");
    open(&f).stash_hunks(&patch, &patch, "").unwrap();
    f.git(&["checkout", "--", "a.txt"]).unwrap();

    f.git(&["stash", "apply"]).unwrap();

    assert_eq!(read(&f, "a.txt"), TEN.replace("two", "TWO"));
}

fn untouched(f: &test_fixtures::Fixture, attempt: impl FnOnce(&RepoHandle)) {
    let status = f.git(&["status", "--porcelain=v2"]).unwrap();
    let staged = f.git(&["diff", "--cached"]).unwrap();
    let unstaged = f.git(&["diff"]).unwrap();
    let stashes = f.git(&["stash", "list"]).unwrap();

    attempt(&open(f));

    assert_eq!(f.git(&["status", "--porcelain=v2"]).unwrap(), status);
    assert_eq!(f.git(&["diff", "--cached"]).unwrap(), staged);
    assert_eq!(f.git(&["diff"]).unwrap(), unstaged);
    assert_eq!(f.git(&["stash", "list"]).unwrap(), stashes);
}

#[test]
fn lines_that_do_not_match_the_files_change_nothing() {
    let f = committed(&["a.txt"]);
    f.write_file("a.txt", &TEN.replace("two", "TWO")).unwrap();
    let stale = "--- a/a.txt\n+++ b/a.txt\n@@ -3,3 +3,3 @@\n three\n-four\n+FOUR\n five\n";

    untouched(&f, |repo| {
        assert!(repo.stash_hunks(stale, stale, "").is_err());
    });
}

#[test]
fn lines_built_on_staged_ones_are_refused_untouched() {
    let f = committed(&["a.txt"]);
    f.write_file("a.txt", &TEN.replace("two", "TWO")).unwrap();
    f.git(&["add", "a.txt"]).unwrap();
    f.write_file(
        "a.txt",
        &TEN.replace("two", "TWO").replace("three", "THREE"),
    )
    .unwrap();
    let patch = "--- a/a.txt\n+++ b/a.txt\n@@ -2,2 +2,2 @@\n TWO\n-three\n+THREE\n";

    untouched(&f, |repo| {
        assert!(repo.stash_hunks(patch, patch, "").is_err());
    });
}

#[test]
fn a_working_tree_that_moved_on_changes_nothing() {
    let f = committed(&["a.txt"]);
    f.write_file("a.txt", &TEN.replace("two", "TWO")).unwrap();
    let stage = second_changed("a.txt");
    let moved = "--- a/a.txt\n+++ b/a.txt\n@@ -1,3 +1,3 @@\n one\n-two\n+Two\n three\n";

    untouched(&f, |repo| {
        assert!(repo.stash_hunks(&stage, moved, "").is_err());
    });
}

#[test]
fn an_empty_selection_changes_nothing() {
    let f = committed(&["a.txt"]);
    f.write_file("a.txt", &TEN.replace("two", "TWO")).unwrap();

    untouched(&f, |repo| {
        assert!(repo.stash_hunks("", "", "").is_err());
    });
}

#[test]
fn nothing_committed_yet_is_refused() {
    let f = test_fixtures::Fixture::init().unwrap();
    f.write_file("a.txt", TEN).unwrap();
    f.git(&["add", "a.txt"]).unwrap();
    let patch = "--- /dev/null\n+++ b/a.txt\n@@ -0,0 +1,1 @@\n+one\n";

    untouched(&f, |repo| {
        assert!(repo.stash_hunks(patch, patch, "").is_err());
    });
}

#[test]
fn undo_puts_the_lines_back_and_takes_the_stash_off() {
    let f = committed(&["a.txt"]);
    let edited = TEN.replace("two", "TWO").replace("nine", "NINE");
    f.write_file("a.txt", &edited).unwrap();
    let patch = second_changed("a.txt");
    let repo = open(&f);
    let stash = repo.stash_hunks(&patch, &patch, "").unwrap();

    repo.unstash_hunks(&stash, &patch).unwrap();

    assert_eq!(read(&f, "a.txt"), edited);
    assert!(repo.stashes().unwrap().is_empty());
}

#[test]
fn undo_after_the_stash_was_applied_is_refused() {
    let f = committed(&["a.txt"]);
    f.write_file("a.txt", &TEN.replace("two", "TWO")).unwrap();
    let patch = second_changed("a.txt");
    let repo = open(&f);
    let stash = repo.stash_hunks(&patch, &patch, "").unwrap();
    f.git(&["stash", "pop"]).unwrap();

    untouched(&f, |repo| {
        assert!(repo.unstash_hunks(&stash, &patch).is_err());
    });
}

#[test]
fn a_new_file_taken_whole_leaves_the_folder() {
    let f = committed(&["a.txt"]);
    f.write_file("new.txt", "fresh\n").unwrap();
    let patch = "diff --git a/new.txt b/new.txt\nnew file mode 100644\n--- /dev/null\n+++ b/new.txt\n@@ -0,0 +1,1 @@\n+fresh\n";

    let stash = open(&f).stash_hunks(patch, patch, "").unwrap();

    assert!(!f.path().join("new.txt").exists());
    assert_eq!(show(&f, &format!("{stash}:new.txt")), "fresh\n");
    f.git(&["stash", "pop"]).unwrap();
    assert_eq!(read(&f, "new.txt"), "fresh\n");
}
