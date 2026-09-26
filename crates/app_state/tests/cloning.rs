//! Repository ▸ Clone… through the state: its own lane, the journal, the footer's Cancel.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, OperationKind};
use git_engine::CloneRequest;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn request(source: &str, target: &Path) -> CloneRequest {
    CloneRequest {
        source: source.to_owned(),
        target: target.to_string_lossy().into_owned(),
        submodules: false,
        all_branches: true,
        branch: None,
        skip_larger_than_mb: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_clone_waits_in_a_lane_of_its_own_and_is_journalled() {
    let f = test_fixtures::linear(2).unwrap();
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("app");
    let state = AppState::new();
    let wanted = request(&f.path().to_string_lossy(), &target);

    let permit = state.enqueue_clone(&wanted.target).await;
    let queued = state.operations();
    let run = state.network_stop(permit.id());
    let result = state.clone_repository(&wanted, &run.token(), |_| {});
    drop(run);
    permit.finish(result.is_ok());

    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].kind, OperationKind::Clone);
    assert_eq!(queued[0].repo, None, "no repository yet");
    assert_eq!(result.unwrap(), target);
    assert!(state.operations().is_empty());
    let entry = &state.command_log()[0];
    assert!(entry.command.starts_with("git clone"), "{}", entry.command);
    assert_eq!(Path::new(&entry.repo), target);
}

#[test]
fn the_check_lists_what_the_server_has() {
    let f = test_fixtures::branched().unwrap();
    let state = AppState::new();

    let found = state.remote_branches(&f.path().to_string_lossy()).unwrap();

    assert_eq!(found.default_branch.as_deref(), Some("main"));
    assert_eq!(found.branches, ["dev", "main"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_footer_s_cancel_stops_a_clone() {
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = server.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let held: Vec<_> = server.incoming().take(4).collect();
        std::thread::sleep(Duration::from_secs(120));
        drop(held);
    });
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("app");
    let state = Arc::new(AppState::new());
    let wanted = request(&format!("git://127.0.0.1:{port}/x.git"), &target);

    let permit = state.enqueue_clone(&wanted.target).await;
    let operation = permit.id();
    let run = state.network_stop(operation);
    let (done, finished) = std::sync::mpsc::channel();
    {
        let (state, stop) = (Arc::clone(&state), run.token());
        std::thread::spawn(move || {
            let _ = done.send(state.clone_repository(&wanted, &stop, |_| {}));
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
        .expect("the clone was still running 20 s after it was cancelled");
    drop(run);
    permit.finish(result.is_ok());

    assert!(
        matches!(result, Err(git_engine::GitError::Cancelled(_))),
        "{result:?}"
    );
    assert!(state.operations().is_empty(), "the lane is free again");
    assert_eq!(state.command_log()[0].summary, "Cancelled by the user");
}
