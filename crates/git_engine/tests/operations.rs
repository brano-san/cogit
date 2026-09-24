// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{RepoHandle, RepoState};

use std::sync::{Arc, Mutex};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

type Log = Arc<Mutex<Vec<String>>>;

fn open_logged(f: &test_fixtures::Fixture) -> (RepoHandle, Log) {
    let log: Log = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let repo = RepoHandle::open(f.path()).unwrap().with_journal(Arc::new(
        move |out: git_engine::GitOutput| {
            if let Ok(mut entries) = sink.lock() {
                entries.push(out.command);
            }
        },
    ));
    (repo, log)
}

fn commands(log: &Log) -> Vec<String> {
    log.lock()
        .map(|entries| entries.clone())
        .unwrap_or_default()
}

#[test]
fn aborting_a_merge_returns_the_repository_to_clean() {
    let f = test_fixtures::conflicted().unwrap();
    let repo = open(&f);
    assert_eq!(repo.state().unwrap(), RepoState::Merging);

    repo.abort_operation().unwrap();

    assert_eq!(repo.state().unwrap(), RepoState::Clean);
}

#[test]
fn aborting_a_merge_restores_the_files_it_touched() {
    let f = test_fixtures::conflicted().unwrap();
    let repo = open(&f);

    repo.abort_operation().unwrap();

    let files = repo.worktree_files().unwrap();
    assert!(
        files.staged.is_empty() && files.unstaged.is_empty(),
        "an aborted merge leaves nothing behind: {files:?}"
    );
}

#[test]
fn aborting_with_nothing_in_progress_is_refused_before_git_is_started() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);

    let err = repo.abort_operation().unwrap_err();

    assert!(
        matches!(err, git_engine::GitError::InvalidState(_)),
        "got {err:?}"
    );
}

#[test]
fn aborting_with_nothing_in_progress_starts_no_process() {
    let f = test_fixtures::linear(2).unwrap();
    let (repo, log) = open_logged(&f);

    let _ = repo.abort_operation();

    assert!(commands(&log).is_empty(), "got {:?}", commands(&log));
}

#[test]
fn continuing_with_nothing_in_progress_is_refused() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);

    assert!(repo.continue_operation().is_err());
}

#[test]
fn continuing_an_unresolved_merge_reports_gits_own_refusal() {
    let f = test_fixtures::conflicted().unwrap();
    let repo = open(&f);

    let err = repo.continue_operation().unwrap_err();

    match err {
        git_engine::GitError::Command(details) => {
            assert!(
                !details.stderr.is_empty() || !details.stdout.is_empty(),
                "{details:?}"
            );
        }
        other => panic!("expected Git to explain itself, got {other:?}"),
    }
}

#[test]
fn a_resolved_merge_can_be_continued() {
    let f = test_fixtures::conflicted().unwrap();
    let repo = open(&f);
    let conflicted: Vec<String> = repo
        .worktree_files()
        .unwrap()
        .unstaged
        .into_iter()
        .filter(|e| e.status == git_engine::FileStatus::Conflicted)
        .map(|e| e.path)
        .collect();
    assert!(!conflicted.is_empty());
    for path in &conflicted {
        std::fs::write(f.path().join(path), "resolved by hand\n").unwrap();
    }
    repo.stage(&conflicted).unwrap();

    repo.continue_operation().unwrap();

    assert_eq!(repo.state().unwrap(), RepoState::Clean);
}

#[test]
fn aborting_a_cherry_pick_uses_the_right_command() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::write(f.git_dir().join("CHERRY_PICK_HEAD"), "0".repeat(40)).unwrap();
    let (repo, log) = open_logged(&f);

    let _ = repo.abort_operation();

    assert!(
        commands(&log).iter().any(|c| c.contains("cherry-pick")),
        "got {:?}",
        commands(&log)
    );
}

#[test]
fn aborting_a_rebase_uses_the_right_command() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::create_dir_all(f.git_dir().join("rebase-merge")).unwrap();
    let (repo, log) = open_logged(&f);

    let _ = repo.abort_operation();

    assert!(
        commands(&log).iter().any(|c| c.contains("rebase")),
        "got {:?}",
        commands(&log)
    );
}

#[test]
fn aborting_git_am_uses_the_right_command() {
    let f = test_fixtures::linear(2).unwrap();
    let apply = f.git_dir().join("rebase-apply");
    std::fs::create_dir_all(&apply).unwrap();
    std::fs::write(apply.join("applying"), "").unwrap();
    let (repo, log) = open_logged(&f);

    let _ = repo.abort_operation();

    assert!(
        commands(&log).iter().any(|c| c.contains("am --abort")),
        "got {:?}",
        commands(&log)
    );
}

/// `git bisect` has no `--abort` and no `--continue`: it ends with `reset`.
#[test]
fn aborting_a_bisect_resets_it() {
    let f = test_fixtures::linear(3).unwrap();
    std::fs::write(f.git_dir().join("BISECT_LOG"), "git bisect start\n").unwrap();
    let (repo, log) = open_logged(&f);

    let _ = repo.abort_operation();

    assert!(
        commands(&log).iter().any(|c| c.contains("bisect reset")),
        "got {:?}",
        commands(&log)
    );
}

#[test]
fn continuing_a_bisect_is_refused_before_git_is_started() {
    let f = test_fixtures::linear(3).unwrap();
    std::fs::write(f.git_dir().join("BISECT_LOG"), "git bisect start\n").unwrap();
    let (repo, log) = open_logged(&f);

    assert!(repo.continue_operation().is_err());
    assert!(commands(&log).is_empty(), "got {:?}", commands(&log));
}

// `git rebase -r` stopped on a conflicting merge commit leaves MERGE_HEAD beside
// rebase-merge. Git calls that a rebase; reading it as a merge made Abort run
// `merge --abort` and left the rebase hanging.
#[test]
fn a_merge_stopped_inside_a_rebase_is_a_rebase() {
    let f = test_fixtures::linear(2).unwrap();
    let git_dir = f.path().join(".git");
    std::fs::create_dir_all(git_dir.join("rebase-merge")).unwrap();
    std::fs::write(
        git_dir.join("MERGE_HEAD"),
        "0000000000000000000000000000000000000000\n",
    )
    .unwrap();

    let state = git_engine::RepoHandle::open(f.path())
        .unwrap()
        .state()
        .unwrap();

    assert_eq!(state, git_engine::RepoState::Rebasing);
}
