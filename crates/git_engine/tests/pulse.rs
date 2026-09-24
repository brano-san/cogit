// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The light status of a row of the Repositories list (R-353): tracking from the local
//! remote-tracking refs, changes from the index's stat data, nothing hashed.

use git_engine::{RepoHandle, pulse};

#[test]
fn a_clean_repository_without_a_remote() {
    let f = test_fixtures::linear(2).unwrap();

    let found = pulse(f.path());

    assert!(!found.missing);
    assert_eq!(found.branch.as_deref(), Some("main"));
    assert!(!found.tracked);
    assert_eq!((found.ahead, found.behind), (0, 0));
    assert!(!found.dirty);
}

#[test]
fn tracking_comes_from_the_local_remote_tracking_ref() {
    let f = test_fixtures::with_remote().unwrap();

    let found = pulse(f.path());

    assert!(found.tracked);
    assert_eq!((found.ahead, found.behind), (2, 1));
}

#[test]
fn a_file_of_another_size_is_a_change() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::write(
        f.path().join("file0.txt"),
        "a longer line than it was before\n",
    )
    .unwrap();

    assert!(pulse(f.path()).dirty);
}

// Same size, written later: only the modification time says so, and that is enough.
#[test]
fn a_rewritten_file_of_the_same_size_is_a_change() {
    let f = test_fixtures::linear(2).unwrap();
    let path = f.path().join("file0.txt");
    let before = std::fs::read(&path).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let same_size: Vec<u8> = before
        .iter()
        .map(|b| if *b == b'\n' { b'\n' } else { b'x' })
        .collect();
    std::fs::write(&path, same_size).unwrap();

    assert!(pulse(f.path()).dirty);
}

#[test]
fn a_deleted_file_is_a_change() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::remove_file(f.path().join("file0.txt")).unwrap();

    assert!(pulse(f.path()).dirty);
}

#[test]
fn a_staged_change_is_a_change_even_with_the_worktree_matching_the_index() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::write(f.path().join("file0.txt"), "staged content\n").unwrap();
    f.git(&["add", "--", "file0.txt"]).unwrap();

    assert!(pulse(f.path()).dirty);
}

// The cheap check cannot see these without walking the directories; the full status of
// the repository on screen does.
#[test]
fn an_untracked_file_is_not_looked_for() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::write(f.path().join("new.txt"), "untracked\n").unwrap();

    assert!(!pulse(f.path()).dirty);
}

#[test]
fn a_detached_head_has_no_branch_and_no_tracking() {
    let f = test_fixtures::detached_head().unwrap();

    let found = pulse(f.path());

    assert!(found.branch.is_none());
    assert!(!found.tracked);
}

#[test]
fn a_folder_that_is_gone_is_missing() {
    let dir = tempfile::tempdir().unwrap();
    let found = pulse(&dir.path().join("gone"));

    assert!(found.missing);
    assert!(!found.dirty);
}

#[test]
fn a_background_fetch_brings_the_remote_tracking_ref() {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["update-ref", "-d", "refs/remotes/origin/main"])
        .unwrap();
    assert!(!pulse(f.path()).tracked);

    RepoHandle::open(f.path())
        .unwrap()
        .background_fetch()
        .unwrap();

    let found = pulse(f.path());
    assert!(found.tracked);
    assert_eq!(found.behind, 1);
}

#[test]
fn a_background_fetch_without_remotes_does_nothing() {
    let f = test_fixtures::linear(2).unwrap();
    RepoHandle::open(f.path())
        .unwrap()
        .background_fetch()
        .unwrap();
}

#[test]
fn a_background_fetch_from_a_remote_that_is_gone_fails() {
    let f = test_fixtures::linear(2).unwrap();
    let gone = f.path().join("no-such-remote.git");
    f.git(&["remote", "add", "origin", &gone.to_string_lossy()])
        .unwrap();

    assert!(
        RepoHandle::open(f.path())
            .unwrap()
            .background_fetch()
            .is_err()
    );
}

/// A measurement on the benchmark's `large` set, run by hand: `COGIT_BENCH_LARGE=… cargo
/// nextest run -p git_engine --test pulse --run-ignored only --no-capture`.
#[test]
#[ignore = "needs the benchmark's large repository in COGIT_BENCH_LARGE"]
#[allow(clippy::print_stderr)]
fn a_pulse_of_the_large_set_takes() {
    let Some(path) = std::env::var_os("COGIT_BENCH_LARGE") else {
        return;
    };
    let path = std::path::PathBuf::from(path);
    let mut times = Vec::new();
    for _ in 0..5 {
        let started = std::time::Instant::now();
        let found = pulse(&path);
        times.push(started.elapsed().as_micros());
        assert!(!found.missing);
    }
    times.sort_unstable();
    eprintln!(
        "pulse of {}: median {} µs, all {times:?}",
        path.display(),
        times[2]
    );
}
