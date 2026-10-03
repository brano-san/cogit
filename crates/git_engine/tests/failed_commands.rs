// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! A command that fails is a `GitCommandError` in the journal, wherever it was spawned.

use std::sync::{Arc, Mutex};

use git_engine::{GitError, GitOutput, RepoHandle};

fn journaled(f: &test_fixtures::Fixture) -> (RepoHandle, Arc<Mutex<Vec<GitOutput>>>) {
    let log: Arc<Mutex<Vec<GitOutput>>> = Arc::default();
    let sink = Arc::clone(&log);
    let repo = RepoHandle::open(f.path())
        .unwrap()
        .with_journal(Arc::new(move |out: GitOutput| {
            sink.lock().unwrap().push(out);
        }));
    (repo, log)
}

#[test]
fn a_failing_textconv_is_a_command_error_with_its_exit_code() {
    let f = test_fixtures::linear(1).unwrap();
    let (repo, log) = journaled(&f);

    let Err(GitError::Command(error)) = repo.textconv("false", "a.bin", b"x") else {
        panic!("expected a command error");
    };

    assert_eq!(error.exit_code, Some(1));
    assert_eq!(log.lock().unwrap().len(), 1);
}

#[test]
fn a_check_ignore_that_fails_is_a_command_error() {
    let f = test_fixtures::linear(1).unwrap();
    let (repo, log) = journaled(&f);

    let Err(GitError::Command(error)) = repo.ignore_rules(&["../outside".to_owned()]) else {
        panic!("expected a command error");
    };

    assert_eq!(error.exit_code, Some(128));
    assert_eq!(log.lock().unwrap().len(), 1);
}
