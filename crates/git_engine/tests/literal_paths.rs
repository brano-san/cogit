// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! A path the user picked is a file name, not a pattern. Only discard and stage learned
//! that (B-14); everywhere else `test[1].txt` still also meant `test1.txt`, because `[1]`
//! is a character class to git.

use git_engine::{CommitRequest, ConflictSide, RepoHandle};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

/// `test1.txt` and `test[1].txt`, both committed.
fn twins() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(1).unwrap();
    f.commit_file(2, "test1.txt", "one\n").unwrap();
    f.commit_file(3, "test[1].txt", "bracket\n").unwrap();
    f
}

fn staged_names(f: &test_fixtures::Fixture) -> Vec<String> {
    f.git(&["diff", "--cached", "--name-only"])
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

// `ls-files --stage -- test[1].txt` listed `test1.txt` first, and its blob went into the
// index under the other name.
#[test]
fn the_exec_bit_keeps_the_file_its_own_content() {
    let f = twins();

    open(&f).stage_mode("test[1].txt", true).unwrap();

    assert_eq!(f.git(&["show", ":test[1].txt"]).unwrap(), "bracket\n");
    assert_eq!(staged_names(&f), ["test[1].txt"]);
}

#[test]
fn resolving_a_conflict_stages_only_that_file() {
    let f = twins();
    f.git(&["checkout", "-q", "-b", "other"]).unwrap();
    f.commit_file(4, "test[1].txt", "theirs\n").unwrap();
    f.git(&["checkout", "-q", "-"]).unwrap();
    f.commit_file(5, "test[1].txt", "ours\n").unwrap();
    assert!(
        f.git(&["merge", "other"]).is_err(),
        "the merge must conflict"
    );
    std::fs::write(f.path().join("test1.txt"), "edited, not to be staged\n").unwrap();

    open(&f)
        .resolve_with("test[1].txt", ConflictSide::Ours)
        .unwrap();

    let staged = f.git(&["diff", "--cached", "--name-only"]).unwrap();
    assert!(!staged.lines().any(|name| name == "test1.txt"), "{staged}");
}

#[test]
fn committing_only_a_file_commits_only_that_file() {
    let f = twins();
    std::fs::write(f.path().join("test1.txt"), "one, edited\n").unwrap();
    std::fs::write(f.path().join("test[1].txt"), "bracket, edited\n").unwrap();
    f.git(&["add", "-A"]).unwrap();

    open(&f)
        .commit(&CommitRequest {
            message: "only the bracket".to_owned(),
            amend: false,
            no_verify: false,
            only: vec!["test[1].txt".to_owned()],
        })
        .unwrap();

    let committed = f
        .git(&["show", "--name-only", "--format=", "HEAD"])
        .unwrap();
    assert_eq!(committed.trim(), "test[1].txt");
}

#[test]
fn a_file_log_lists_only_that_file() {
    let f = twins();

    let log = open(&f).file_log("test[1].txt", None, false, 100).unwrap();

    assert_eq!(log.len(), 1, "only the commit that added it: {log:?}");
}

// `commit --only` put every path on the command line; past about 32 000 characters
// Windows refuses to start the process (R-191).
#[test]
fn committing_only_thousands_of_files_fits_the_command_line() {
    let f = test_fixtures::linear(1).unwrap();
    let names: Vec<String> = (0..2500).map(|i| format!("dir/file-{i:05}.txt")).collect();
    std::fs::create_dir_all(f.path().join("dir")).unwrap();
    for name in &names {
        std::fs::write(f.path().join(name), "x\n").unwrap();
    }
    f.git(&["add", "-A"]).unwrap();

    open(&f)
        .commit(&CommitRequest {
            message: "many".to_owned(),
            amend: false,
            no_verify: false,
            only: names,
        })
        .unwrap();

    let committed = f
        .git(&["show", "--name-only", "--format=", "HEAD"])
        .unwrap();
    assert_eq!(committed.lines().count(), 2500);
}
