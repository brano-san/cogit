use git_engine::{GitError, RepoHandle, children};
use std::time::{Duration, Instant};

/// One test in a binary of its own: stopping is final for the whole process.
#[test]
fn exiting_stops_the_git_tree_that_runs_and_refuses_the_next() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    let started = Instant::now();
    let sleeper = std::thread::spawn(move || repo.run_git(&["-c", "alias.nap=!sleep 30", "nap"]));

    while children::running() == 0 {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "git never started"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(children::stop_all() >= 1);

    let result = sleeper.join().unwrap();
    assert!(result.is_err(), "a stopped run is a failed run: {result:?}");
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "git → sh → sleep: the grandchild holds the pipes, so the whole tree must stop: {:?}",
        started.elapsed()
    );
    assert_eq!(children::running(), 0);

    let refused = RepoHandle::open(f.path()).unwrap().run_git(&["status"]);
    assert!(matches!(refused, Err(GitError::Io(_))), "{refused:?}");
}
