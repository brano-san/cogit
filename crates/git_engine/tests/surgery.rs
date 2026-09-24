// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::RepoHandle;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

/// Three commits, each rewriting `file.txt`, so every version has a known content.
fn versioned() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(1).unwrap();
    for step in 1..=3 {
        f.write_file("file.txt", &format!("version {step}\n"))
            .unwrap();
        f.git(&["add", "--", "file.txt"]).unwrap();
        f.commit_staged(step, &format!("write version {step}"))
            .unwrap();
    }
    f
}

fn read(f: &test_fixtures::Fixture, name: &str) -> String {
    std::fs::read_to_string(f.path().join(name)).unwrap()
}

#[test]
fn a_file_comes_back_from_an_older_commit() {
    let f = versioned();
    let older = f.oid("HEAD~2").unwrap();

    open(&f)
        .rollback_to(&older, &["file.txt".to_owned()])
        .unwrap();

    assert_eq!(read(&f, "file.txt"), "version 1\n");
}

#[test]
fn rolling_back_does_not_move_head() {
    let f = versioned();
    let head = f.oid("HEAD").unwrap();

    open(&f)
        .rollback_to(&f.oid("HEAD~2").unwrap(), &["file.txt".to_owned()])
        .unwrap();

    assert_eq!(f.oid("HEAD").unwrap(), head);
}

#[test]
fn rolling_back_leaves_the_branch_checked_out() {
    let f = versioned();

    open(&f)
        .rollback_to(&f.oid("HEAD~1").unwrap(), &["file.txt".to_owned()])
        .unwrap();

    assert!(matches!(
        open(&f).head().unwrap(),
        git_engine::Head::Branch { .. }
    ));
}

#[test]
fn the_restored_content_shows_up_as_an_uncommitted_change() {
    let f = versioned();

    open(&f)
        .rollback_to(&f.oid("HEAD~2").unwrap(), &["file.txt".to_owned()])
        .unwrap();

    let files = open(&f).worktree_files().unwrap();
    assert!(
        files
            .unstaged
            .iter()
            .chain(files.staged.iter())
            .any(|entry| entry.path == "file.txt"),
        "{files:?}"
    );
}

#[test]
fn an_empty_path_list_restores_the_whole_tree() {
    let f = versioned();
    f.write_file("second.txt", "new\n").unwrap();
    f.git(&["add", "--", "second.txt"]).unwrap();
    f.commit_staged(4, "add a second file").unwrap();

    open(&f)
        .rollback_to(&f.oid("HEAD~1").unwrap(), &[])
        .unwrap();

    assert_eq!(read(&f, "file.txt"), "version 3\n");
}

#[test]
fn rolling_back_to_a_revision_that_does_not_exist_is_an_error() {
    let f = versioned();
    assert!(
        open(&f)
            .rollback_to("no-such-rev", &["file.txt".to_owned()])
            .is_err()
    );
}

#[test]
fn rolling_back_a_path_absent_from_that_commit_is_an_error_not_a_deletion() {
    let f = versioned();
    let root = f.oid("HEAD~2").unwrap();

    assert!(
        open(&f)
            .rollback_to(&root, &["never-existed.txt".to_owned()])
            .is_err()
    );
}

/// One commit touching five files, on top of a root commit.
fn wide() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(1).unwrap();
    for name in ["a.txt", "b.txt", "c.txt", "d.txt", "e.txt"] {
        f.write_file(name, &format!("{name} content\n")).unwrap();
        f.git(&["add", "--", name]).unwrap();
    }
    f.commit_staged(2, "add five files").unwrap();
    f
}

fn tree_of(f: &test_fixtures::Fixture, rev: &str) -> String {
    f.git(&["rev-parse", &format!("{rev}^{{tree}}")]).unwrap()
}

#[test]
fn splitting_off_files_produces_two_commits() {
    let f = wide();
    let target = f.oid("HEAD").unwrap();
    let before = f.git(&["rev-list", "--count", "HEAD"]).unwrap();

    open(&f)
        .split_off(
            &target,
            &["a.txt".to_owned(), "b.txt".to_owned()],
            "split: a and b",
            true,
        )
        .unwrap();

    let after = f.git(&["rev-list", "--count", "HEAD"]).unwrap();
    assert_eq!(
        after.trim().parse::<u32>().unwrap(),
        before.trim().parse::<u32>().unwrap() + 1
    );
}

#[test]
fn splitting_off_does_not_change_the_resulting_tree_by_a_single_byte() {
    let f = wide();
    let target = f.oid("HEAD").unwrap();
    let tree = tree_of(&f, "HEAD");

    open(&f)
        .split_off(
            &target,
            &["a.txt".to_owned(), "b.txt".to_owned(), "c.txt".to_owned()],
            "split: a, b and c",
            true,
        )
        .unwrap();

    assert_eq!(tree_of(&f, "HEAD"), tree);
}

#[test]
fn the_split_commit_comes_first_when_asked_for() {
    let f = wide();
    let target = f.oid("HEAD").unwrap();

    open(&f)
        .split_off(&target, &["a.txt".to_owned()], "split: a", true)
        .unwrap();

    let messages = f.git(&["log", "--format=%s", "-2"]).unwrap();
    let lines: Vec<&str> = messages.lines().collect();
    assert_eq!(lines[1], "split: a", "{messages}");
}

#[test]
fn the_split_commit_comes_last_when_asked_for() {
    let f = wide();
    let target = f.oid("HEAD").unwrap();

    open(&f)
        .split_off(&target, &["a.txt".to_owned()], "split: a", false)
        .unwrap();

    let messages = f.git(&["log", "--format=%s", "-2"]).unwrap();
    let lines: Vec<&str> = messages.lines().collect();
    assert_eq!(lines[0], "split: a", "{messages}");
}

#[test]
fn the_split_commit_carries_only_the_chosen_files() {
    let f = wide();
    let target = f.oid("HEAD").unwrap();

    open(&f)
        .split_off(&target, &["a.txt".to_owned()], "split: a", true)
        .unwrap();

    let files = f
        .git(&["show", "--name-only", "--format=", "HEAD~1"])
        .unwrap();
    assert_eq!(files.trim(), "a.txt", "{files}");
}

#[test]
fn later_commits_survive_the_rewrite() {
    let f = wide();
    let target = f.oid("HEAD").unwrap();
    f.write_file("later.txt", "later\n").unwrap();
    f.git(&["add", "--", "later.txt"]).unwrap();
    f.commit_staged(3, "a commit after the target").unwrap();
    let tree = tree_of(&f, "HEAD");

    open(&f)
        .split_off(&target, &["a.txt".to_owned()], "split: a", true)
        .unwrap();

    assert_eq!(tree_of(&f, "HEAD"), tree);
    assert!(
        f.git(&["log", "--format=%s"])
            .unwrap()
            .contains("a commit after the target")
    );
}

#[test]
fn splitting_off_every_file_is_refused() {
    let f = wide();
    let target = f.oid("HEAD").unwrap();
    let all = ["a.txt", "b.txt", "c.txt", "d.txt", "e.txt"]
        .map(str::to_owned)
        .to_vec();

    assert!(
        open(&f)
            .split_off(&target, &all, "everything", true)
            .is_err()
    );
}

#[test]
fn splitting_off_no_file_is_refused() {
    let f = wide();
    let target = f.oid("HEAD").unwrap();

    assert!(open(&f).split_off(&target, &[], "nothing", true).is_err());
}

#[test]
fn a_path_the_commit_does_not_touch_is_refused() {
    let f = wide();
    let target = f.oid("HEAD").unwrap();

    assert!(
        open(&f)
            .split_off(&target, &["file0.txt".to_owned()], "untouched", true)
            .is_err()
    );
}

#[test]
fn splitting_off_a_merge_commit_is_refused() {
    let f = test_fixtures::diamond().unwrap();
    let merge = f.oid("HEAD").unwrap();

    assert!(
        open(&f)
            .split_off(&merge, &["a.txt".to_owned()], "from a merge", true)
            .is_err()
    );
}

#[test]
fn a_dirty_working_tree_stops_the_rewrite_before_it_starts() {
    let f = wide();
    let target = f.oid("HEAD").unwrap();
    f.write_file("a.txt", "uncommitted edit\n").unwrap();

    assert!(
        open(&f)
            .split_off(&target, &["a.txt".to_owned()], "split: a", true)
            .is_err()
    );
}

#[test]
fn a_commit_only_on_the_local_branch_is_not_published() {
    let f = test_fixtures::with_remote().unwrap();
    f.write_file("local.txt", "local\n").unwrap();
    f.git(&["add", "--", "local.txt"]).unwrap();
    let oid = f.commit_staged(9, "a local commit").unwrap();

    assert!(!open(&f).is_published(&oid).unwrap());
}

#[test]
fn a_commit_the_remote_already_has_is_published() {
    let f = test_fixtures::with_remote().unwrap();
    f.write_file("shared.txt", "shared\n").unwrap();
    f.git(&["add", "--", "shared.txt"]).unwrap();
    let oid = f.commit_staged(9, "a shared commit").unwrap();
    f.git(&["push", "origin", "HEAD:refs/heads/shared"])
        .unwrap();
    f.git(&["fetch", "origin"]).unwrap();

    assert!(open(&f).is_published(&oid).unwrap());
}

#[test]
fn a_repository_without_a_remote_has_nothing_published() {
    let f = test_fixtures::linear(2).unwrap();
    assert!(!open(&f).is_published(&f.oid("HEAD").unwrap()).unwrap());
}

// With git's default `core.quotepath`, `--name-only` prints a non-ASCII path quoted and
// escaped, so a Cyrillic file chosen in the dialog was "not one of the files this commit
// changed" and could not be split off.
#[test]
fn a_file_with_a_non_ascii_name_can_be_split_off() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["config", "core.quotepath", "true"]).unwrap();
    for name in ["отчёт.txt", "plain.txt"] {
        f.write_file(name, &format!("{name} content\n")).unwrap();
        f.git(&["add", "--", name]).unwrap();
    }
    f.commit_staged(2, "add two files").unwrap();
    let target = f.oid("HEAD").unwrap();

    open(&f)
        .split_off(
            &target,
            &["отчёт.txt".to_owned()],
            "split: the report",
            true,
        )
        .unwrap();
}

// The branch was rebased onto the split without --rebase-merges: every merge after the
// split commit was flattened into a line.
#[test]
fn a_merge_after_the_split_commit_stays_a_merge() {
    let f = wide();
    let target = f.oid("HEAD").unwrap();
    f.git(&["switch", "-q", "-c", "side"]).unwrap();
    f.commit_file(3, "side.txt", "side\n").unwrap();
    f.git(&["switch", "-q", "-"]).unwrap();
    f.commit_file(4, "main.txt", "main\n").unwrap();
    f.merge(5, &["side"], "merge side").unwrap();

    open(&f)
        .split_off(&target, &["a.txt".to_owned()], "split: a", true)
        .unwrap();

    let merges = f.git(&["rev-list", "--merges", "--count", "HEAD"]).unwrap();
    assert_eq!(merges.trim(), "1");
}

// The `-z` listing went through the journal's record, which masks what follows `=` on a
// line that names a token or a secret: with no newlines the whole listing is one line.
#[test]
fn file_names_that_look_like_secrets_can_be_split_off() {
    let f = test_fixtures::linear(1).unwrap();
    for name in ["api/token.rs", "data/year=2024/a.csv", "z.txt"] {
        f.write_file(name, "x\n").unwrap();
        f.git(&["add", "--", name]).unwrap();
    }
    f.commit_staged(2, "three files").unwrap();
    let target = f.oid("HEAD").unwrap();

    open(&f)
        .split_off(&target, &["z.txt".to_owned()], "split: z", true)
        .unwrap();
}
