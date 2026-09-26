//! Cancelling a running fetch, pull or push by its operation id (03 §3 п.6).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, OperationKind};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A remote that takes the connection and never says a word.
fn silent_remote(f: &test_fixtures::Fixture) {
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = server.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let held: Vec<_> = server.incoming().take(4).collect();
        std::thread::sleep(Duration::from_secs(120));
        drop(held);
    });
    f.git(&[
        "remote",
        "add",
        "quiet",
        &format!("git://127.0.0.1:{port}/x.git"),
    ])
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cancelled_fetch_ends_journalled_and_frees_the_lane() {
    let f = test_fixtures::linear(1).unwrap();
    silent_remote(&f);
    let state = Arc::new(AppState::new());
    let repo = state.open_repository(f.path()).unwrap().repo;
    let permit = state.enqueue(repo, OperationKind::Fetch, "Fetching").await;
    let operation = permit.id();
    let running = state.network_stop(operation);
    let (done, finished) = std::sync::mpsc::channel();
    {
        let (state, stop) = (Arc::clone(&state), running.token());
        std::thread::spawn(move || {
            let _ = done.send(state.fetch(repo, "quiet", &stop, |_| {}));
        });
    }
    let started = Instant::now();
    while git_engine::children::running() == 0 {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "git never started"
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    assert!(state.cancel_network(operation));
    let result = finished
        .recv_timeout(Duration::from_secs(20))
        .expect("the fetch was still running 20 s after it was cancelled");
    drop(running);
    permit.finish(result.is_ok());

    assert!(
        matches!(result, Err(git_engine::GitError::Cancelled(_))),
        "{result:?}"
    );
    assert!(state.operations().is_empty(), "the lane is free again");
    assert_eq!(state.command_log()[0].summary, "Cancelled by the user");
    assert!(
        !state.cancel_network(operation),
        "nothing is left to cancel"
    );
}

#[test]
fn there_is_nothing_to_cancel_but_a_running_network_operation() {
    let state = AppState::new();
    assert!(!state.cancel_network(7), "no such operation");

    let running = state.network_stop(7);
    assert!(state.cancel_network(7));
    assert!(running.token().is_stopped());
    assert!(!state.cancel_network(7), "asked once already");
    drop(running);
    assert!(!state.cancel_network(7), "it is over");
}
