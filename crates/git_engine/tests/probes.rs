// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! A question whose answer may be "no" — does this branch exist, is this file tracked —
//! is not a failed command. Asked through the journal it was recorded as one, and every
//! journalled failure opens a notification: starting a feature said something went wrong.

use git_engine::{FlowConfig, FlowKind, RepoHandle};
use std::sync::{Arc, Mutex};

/// The handle, and every journalled command that exited non-zero.
fn watched(f: &test_fixtures::Fixture) -> (RepoHandle, Arc<Mutex<Vec<String>>>) {
    let failed: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&failed);
    let repo = RepoHandle::open(f.path()).unwrap().with_journal(Arc::new(
        move |out: git_engine::GitOutput| {
            if out.exit_code != Some(0) {
                sink.lock().unwrap().push(out.command);
            }
        },
    ));
    (repo, failed)
}

#[test]
fn starting_a_feature_records_no_failure() {
    let f = test_fixtures::linear(1).unwrap();
    RepoHandle::open(f.path())
        .unwrap()
        .flow_init(&FlowConfig::default())
        .unwrap();
    let (repo, failed) = watched(&f);

    repo.flow_start(FlowKind::Feature, "x").unwrap();

    assert!(
        failed.lock().unwrap().is_empty(),
        "{:?}",
        failed.lock().unwrap()
    );
}

#[test]
fn moving_an_untracked_file_records_no_failure() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("loose.txt"), "x\n").unwrap();
    let (repo, failed) = watched(&f);

    repo.move_path("loose.txt", "moved.txt").unwrap();

    assert!(
        failed.lock().unwrap().is_empty(),
        "{:?}",
        failed.lock().unwrap()
    );
}

#[test]
fn moving_a_tracked_folder_is_still_a_staged_rename() {
    let f = test_fixtures::linear(1).unwrap();
    f.commit_file(2, "dir/inner.txt", "x\n").unwrap();
    let (repo, _) = watched(&f);

    repo.move_path("dir", "renamed").unwrap();

    let staged = f.git(&["diff", "--cached", "--name-status", "-M"]).unwrap();
    assert!(staged.contains("renamed/inner.txt"), "{staged}");
}

#[test]
fn the_exec_bit_of_an_untracked_file_is_refused_without_a_failed_command() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("loose.sh"), "x\n").unwrap();
    let (repo, failed) = watched(&f);

    assert!(repo.stage_mode("loose.sh", true).is_err());
    assert!(
        failed.lock().unwrap().is_empty(),
        "{:?}",
        failed.lock().unwrap()
    );
}
