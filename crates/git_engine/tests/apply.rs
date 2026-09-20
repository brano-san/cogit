// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::RepoHandle;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

const ONE_LINE: &str =
    "--- a/file0.txt\n+++ b/file0.txt\n@@ -1,1 +1,2 @@\n content 0\n+added line\n";

#[test]
fn a_patch_lands_in_the_index_without_touching_the_working_tree() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(
        f.path().join("file0.txt"),
        "content 0\nadded line\nand more\n",
    )
    .unwrap();
    let repo = open(&f);

    repo.apply_patch(ONE_LINE, false).unwrap();

    let files = repo.worktree_files().unwrap();
    assert_eq!(files.staged.len(), 1, "the selected line is staged");
    assert_eq!(files.unstaged.len(), 1, "the rest stays unstaged");
    assert!(
        std::fs::read_to_string(f.path().join("file0.txt"))
            .unwrap()
            .contains("and more"),
        "the file on disk must not change"
    );
}

#[test]
fn the_staged_content_is_exactly_what_the_patch_described() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(
        f.path().join("file0.txt"),
        "content 0\nadded line\nand more\n",
    )
    .unwrap();
    let repo = open(&f);

    repo.apply_patch(ONE_LINE, false).unwrap();

    let staged = repo
        .diff_sides(&git_engine::DiffSpec::IndexVsHead, "file0.txt")
        .unwrap()
        .1
        .unwrap();
    assert_eq!(
        String::from_utf8(staged).unwrap(),
        "content 0\nadded line\n"
    );
}

#[test]
fn reversing_the_same_patch_unstages_the_line() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("file0.txt"), "content 0\nadded line\n").unwrap();
    let repo = open(&f);
    repo.apply_patch(ONE_LINE, false).unwrap();

    repo.apply_patch(ONE_LINE, true).unwrap();

    assert!(repo.worktree_files().unwrap().staged.is_empty());
}

#[test]
fn a_patch_that_does_not_apply_reports_gits_own_words() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    let err = repo
        .apply_patch(
            "--- a/file0.txt\n+++ b/file0.txt\n@@ -1,1 +1,1 @@\n-something else\n+nope\n",
            false,
        )
        .unwrap_err();

    match err {
        git_engine::GitError::Command(details) => {
            assert!(!details.stderr.is_empty(), "{details:?}");
        }
        other => panic!("expected a command failure, got {other:?}"),
    }
}

#[test]
fn an_empty_patch_is_refused_before_git_is_started() {
    let f = test_fixtures::linear(1).unwrap();
    assert!(open(&f).apply_patch("   ", false).is_err());
}

#[test]
fn a_crlf_file_keeps_its_line_endings_after_partial_staging() {
    let f = test_fixtures::crlf_files().unwrap();
    f.write_file("crlf.txt", "first\r\nsecond\r\nthird\r\nfourth\r\n")
        .unwrap();
    let repo = open(&f);
    let patch = "--- a/crlf.txt\n+++ b/crlf.txt\n@@ -3,1 +3,2 @@\n third\r\n+fourth\r\n";

    repo.apply_patch(patch, false).unwrap();

    let staged = repo
        .diff_sides(&git_engine::DiffSpec::IndexVsHead, "crlf.txt")
        .unwrap()
        .1
        .unwrap();
    let text = String::from_utf8(staged).unwrap();
    assert!(text.contains("fourth\r\n"), "INV-08: got {text:?}");
}

#[test]
fn the_patch_is_logged_before_it_is_applied() {
    use std::sync::{Arc, Mutex};

    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("file0.txt"), "content 0\nadded line\n").unwrap();
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let repo = RepoHandle::open(f.path()).unwrap().with_journal(Arc::new(
        move |out: git_engine::GitOutput| {
            if let Ok(mut entries) = sink.lock() {
                entries.push(out.command);
            }
        },
    ));

    repo.apply_patch(ONE_LINE, false).unwrap();

    let commands = log.lock().unwrap().clone();
    assert!(
        commands.iter().any(|c| c.contains("apply")),
        "R-04: the exact command must be recoverable from the journal, got {commands:?}"
    );
}
