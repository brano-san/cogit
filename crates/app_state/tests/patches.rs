// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, RepoId};
use git_engine::{PatchMode, RepoState};

fn open(f: &test_fixtures::Fixture) -> (AppState, RepoId) {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    (state, repo)
}

fn paths(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn status(f: &test_fixtures::Fixture) -> String {
    f.git(&["status", "--porcelain", "--untracked-files=all"])
        .unwrap()
}

fn text(f: &test_fixtures::Fixture, name: &str) -> String {
    std::fs::read_to_string(f.path().join(name)).unwrap()
}

fn repo_state(f: &test_fixtures::Fixture) -> RepoState {
    git_engine::RepoHandle::open(f.path())
        .unwrap()
        .state()
        .unwrap()
}

/// The working-tree change of `file1.txt`, taken as a patch, with the tree clean again.
fn edit_patch(f: &test_fixtures::Fixture, state: &AppState, repo: RepoId) -> String {
    f.write_file("file1.txt", "patched 1\n").unwrap();
    let patch = state
        .worktree_patch_text(repo, &paths(&["file1.txt"]), false)
        .unwrap();
    f.git(&["checkout", "--", "file1.txt"]).unwrap();
    patch
}

const BINARY: &[u8] = b"\x00\x01\x02binary\xff\xfe";

#[test]
fn one_commit_is_saved_as_one_patch_file() {
    let f = test_fixtures::linear(3).unwrap();
    let (state, repo) = open(&f);
    let out = test_fixtures::tempdir().unwrap();
    let target = out.path().join("one.patch");

    let written = state
        .save_commit_patches(repo, &[f.oid("HEAD").unwrap()], &target, true)
        .unwrap();

    assert_eq!(written.len(), 1);
    let saved = std::fs::read_to_string(&target).unwrap();
    assert!(saved.starts_with(&format!("From {} ", f.oid("HEAD").unwrap())));
    assert!(saved.contains("Subject: [PATCH] commit 2"));
    assert!(saved.contains("+content 2"));
}

#[test]
fn three_commits_go_to_numbered_files_oldest_first_or_to_one() {
    let f = test_fixtures::linear(3).unwrap();
    let (state, repo) = open(&f);
    let out = test_fixtures::tempdir().unwrap();
    // Root commit included, given in click order.
    let commits = vec![
        f.oid("HEAD~1").unwrap(),
        f.oid("HEAD").unwrap(),
        f.oid("HEAD~2").unwrap(),
    ];

    let written = state
        .save_commit_patches(repo, &commits, &out.path().join("series"), false)
        .unwrap();
    let names: Vec<&str> = written
        .iter()
        .map(|path| path.rsplit('/').next().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "0001-commit-0.patch",
            "0002-commit-1.patch",
            "0003-commit-2.patch"
        ]
    );

    let all = state.commit_patch_text(repo, &commits).unwrap();
    assert_eq!(all.matches("\nSubject: [PATCH ").count(), 3);
    assert!(all.contains("[PATCH 1/3] commit 0"));
}

#[test]
fn a_merge_commit_is_refused() {
    let f = test_fixtures::diamond().unwrap();
    let (state, repo) = open(&f);
    let merge = f
        .git(&["rev-list", "--merges", "-n", "1", "--all"])
        .unwrap();
    let merge = merge.trim();
    assert!(!merge.is_empty());

    assert!(state.commit_patch_text(repo, &[merge.to_owned()]).is_err());
}

#[test]
fn unstaged_and_staged_changes_are_saved_with_binaries() {
    let f = test_fixtures::linear(2).unwrap();
    let (state, repo) = open(&f);
    f.write_file("file0.txt", "unstaged 0\n").unwrap();
    std::fs::write(f.path().join("new.bin"), BINARY).unwrap();
    std::fs::write(f.path().join("staged.bin"), BINARY).unwrap();
    f.git(&["add", "staged.bin"]).unwrap();

    let unstaged = state
        .worktree_patch_text(repo, &paths(&["file0.txt", "new.bin"]), false)
        .unwrap();
    assert!(unstaged.contains("+unstaged 0"));
    assert!(unstaged.contains("diff --git a/new.bin b/new.bin"));
    assert!(unstaged.contains("GIT binary patch"));
    assert!(!unstaged.contains("staged.bin"));

    let staged = state
        .worktree_patch_text(repo, &paths(&["staged.bin"]), true)
        .unwrap();
    assert!(staged.contains("diff --git a/staged.bin b/staged.bin"));
    assert!(staged.contains("GIT binary patch"));

    // Binary-safe both ways: the saved patch brings the very bytes back.
    let out = test_fixtures::tempdir().unwrap();
    let target = out.path().join("staged.patch");
    state
        .save_worktree_patch(repo, &paths(&["staged.bin"]), true, &target)
        .unwrap();
    f.git(&["rm", "-q", "--cached", "staged.bin"]).unwrap();
    std::fs::remove_file(f.path().join("staged.bin")).unwrap();
    let patch = app_state::read_patch_file(&target).unwrap();
    state
        .apply_user_patch(repo, &patch, PatchMode::WorkTree)
        .unwrap();
    assert_eq!(std::fs::read(f.path().join("staged.bin")).unwrap(), BINARY);
}

#[test]
fn a_plain_diff_goes_to_the_working_tree_or_to_the_index() {
    let f = test_fixtures::linear(2).unwrap();
    let (state, repo) = open(&f);
    let patch = edit_patch(&f, &state, repo);

    let preview = state
        .patch_preview(repo, &patch, PatchMode::WorkTree)
        .unwrap();
    assert_eq!(preview.problem, None);
    assert_eq!(preview.files.len(), 1);
    assert_eq!(preview.files[0].path, "file1.txt");
    assert_eq!(
        (preview.files[0].added, preview.files[0].removed),
        (Some(1), Some(1))
    );

    state
        .apply_user_patch(repo, &patch, PatchMode::WorkTree)
        .unwrap();
    assert_eq!(status(&f), " M file1.txt\n");

    f.git(&["checkout", "--", "file1.txt"]).unwrap();
    state
        .apply_user_patch(repo, &patch, PatchMode::Index)
        .unwrap();
    assert_eq!(status(&f), "M  file1.txt\n");
    assert_eq!(text(&f, "file1.txt"), "patched 1\n");
}

#[test]
fn a_failed_check_is_told_and_nothing_is_written() {
    let f = test_fixtures::linear(2).unwrap();
    let (state, repo) = open(&f);
    let patch = edit_patch(&f, &state, repo);
    f.commit_file(5, "file1.txt", "moved on\n").unwrap();

    let preview = state
        .patch_preview(repo, &patch, PatchMode::WorkTree)
        .unwrap();
    let problem = preview.problem.expect("the check refuses");
    assert!(problem.contains("patch does not apply"), "{problem}");
    assert_eq!(status(&f), "");

    assert!(
        state
            .apply_user_patch(repo, &patch, PatchMode::WorkTree)
            .is_err()
    );
    assert_eq!(status(&f), "");
    assert_eq!(text(&f, "file1.txt"), "moved on\n");
}

#[test]
fn text_that_is_no_patch_is_a_problem_not_an_error() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);

    let preview = state
        .patch_preview(repo, "just some words\n", PatchMode::WorkTree)
        .unwrap();

    assert!(preview.problem.is_some());
    assert!(preview.files.is_empty());
}

#[test]
fn undo_takes_an_applied_diff_back_out_of_the_tree_and_the_index() {
    let f = test_fixtures::linear(2).unwrap();
    let (state, repo) = open(&f);
    let patch = edit_patch(&f, &state, repo);

    for mode in [PatchMode::WorkTree, PatchMode::Index] {
        state.apply_user_patch(repo, &patch, mode).unwrap();
        assert_ne!(status(&f), "");

        state.undo_last(repo).unwrap();

        assert_eq!(status(&f), "", "{mode:?}");
        assert_eq!(text(&f, "file1.txt"), "content 1\n");
    }
}

/// Patches of the two newest commits, with the branch put back below them.
fn mailbox(f: &test_fixtures::Fixture, state: &AppState, repo: RepoId) -> String {
    let commits = vec![f.oid("HEAD~1").unwrap(), f.oid("HEAD").unwrap()];
    let text = state.commit_patch_text(repo, &commits).unwrap();
    f.git(&["reset", "-q", "--hard", "HEAD~2"]).unwrap();
    text
}

#[test]
fn a_mailbox_becomes_commits_and_undo_puts_the_branch_back() {
    let f = test_fixtures::linear(4).unwrap();
    let (state, repo) = open(&f);
    let mbox = mailbox(&f, &state, repo);
    let before = f.oid("HEAD").unwrap();

    let preview = state
        .patch_preview(repo, &mbox, PatchMode::Mailbox)
        .unwrap();
    assert_eq!(preview.problem, None);
    assert_eq!(preview.files.len(), 2);

    state
        .apply_user_patch(repo, &mbox, PatchMode::Mailbox)
        .unwrap();

    let log = f.git(&["log", "--format=%s", "-3"]).unwrap();
    assert_eq!(log, "commit 3\ncommit 2\ncommit 1\n");
    assert_eq!(text(&f, "file3.txt"), "content 3\n");

    state.undo_last(repo).unwrap();
    assert_eq!(f.oid("HEAD").unwrap(), before);
}

#[test]
fn a_mailbox_that_conflicts_stops_in_am_and_abort_goes_back() {
    let f = test_fixtures::linear(2).unwrap();
    let (state, repo) = open(&f);
    f.commit_file(5, "file1.txt", "theirs\n").unwrap();
    let mbox = state
        .commit_patch_text(repo, &[f.oid("HEAD").unwrap()])
        .unwrap();
    f.git(&["reset", "-q", "--hard", "HEAD~1"]).unwrap();
    f.commit_file(6, "file1.txt", "ours\n").unwrap();
    let before = f.oid("HEAD").unwrap();

    let preview = state
        .patch_preview(repo, &mbox, PatchMode::Mailbox)
        .unwrap();
    assert!(
        preview.conflicts || preview.problem.is_some(),
        "{preview:?}"
    );

    assert!(
        state
            .apply_user_patch(repo, &mbox, PatchMode::Mailbox)
            .is_err()
    );
    assert_eq!(repo_state(&f), RepoState::ApplyingPatches);

    state.abort_operation(repo).unwrap();
    assert_eq!(repo_state(&f), RepoState::Clean);
    assert_eq!(f.oid("HEAD").unwrap(), before);
    assert_eq!(text(&f, "file1.txt"), "ours\n");
}
