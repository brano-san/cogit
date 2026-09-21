//! One lane per repository: everything the user asked for runs, in the order they asked.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppEvent, AppState, OperationKind, OperationPhase, RepoId};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

fn opened() -> (Arc<AppState>, RepoId) {
    let state = Arc::new(AppState::new());
    let repo = state.register(PathBuf::from("/somewhere"), "fixture".into());
    (state, repo)
}

/// Waits for the queue to hold exactly this many operations, so a test never depends on
/// how fast a task happens to be scheduled.
async fn until(state: &AppState, count: usize) {
    for _ in 0..400 {
        if state.operations().len() == count {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("the queue never reached {count} operations");
}

#[tokio::test]
async fn a_second_push_waits_for_the_first() {
    let (state, repo) = opened();
    let first = state.enqueue(repo, OperationKind::Push, "Pushing").await;

    let second = Arc::clone(&state);
    let waiting = tokio::spawn(async move {
        second
            .enqueue(repo, OperationKind::Push, "Pushing")
            .await
            .finish(true);
    });

    until(&state, 2).await;
    assert!(
        !waiting.is_finished(),
        "two pushes at once fight over the ref"
    );

    first.finish(true);
    waiting.await.unwrap();
    assert!(state.operations().is_empty());
}

#[tokio::test]
async fn three_clicks_run_three_times_in_order() {
    let (state, repo) = opened();
    let (done, mut order) = tokio::sync::mpsc::unbounded_channel();
    let held = state
        .enqueue(repo, OperationKind::Tag, "Pushing a tag")
        .await;

    let mut tasks = Vec::new();
    for number in 1usize..=3 {
        let clicked = Arc::clone(&state);
        let done = done.clone();
        tasks.push(tokio::spawn(async move {
            let permit = clicked
                .enqueue(repo, OperationKind::Tag, "Pushing a tag")
                .await;
            let _ = done.send(number);
            permit.finish(true);
        }));
        // Its place is taken by the time the next one asks for one.
        until(&state, 1 + number).await;
    }

    held.finish(true);
    for task in tasks {
        task.await.unwrap();
    }

    let mut seen = Vec::new();
    while let Ok(number) = order.try_recv() {
        seen.push(number);
    }
    assert_eq!(seen, vec![1, 2, 3], "none dropped, none overtaken");
}

#[tokio::test]
async fn two_repositories_do_not_wait_for_each_other() {
    let (state, one) = opened();
    let two = state.register(PathBuf::from("/elsewhere"), "other".into());

    let held = state.enqueue(one, OperationKind::Fetch, "Fetching").await;
    let other = state.enqueue(two, OperationKind::Fetch, "Fetching").await;

    let running = state
        .operations()
        .into_iter()
        .filter(|operation| operation.phase == OperationPhase::Running)
        .count();
    assert_eq!(
        running, 2,
        "a slow fetch in one repository is not the other's problem"
    );

    held.finish(true);
    other.finish(true);
}

#[tokio::test]
async fn a_permit_dropped_on_the_way_out_still_frees_the_lane() {
    let (state, repo) = opened();
    {
        let _abandoned = state.enqueue(repo, OperationKind::Merge, "Merging").await;
    }

    let next = state.enqueue(repo, OperationKind::Merge, "Merging").await;
    assert_eq!(state.operations().len(), 1, "the lane reopened");
    next.finish(true);
}

#[tokio::test]
async fn the_queue_survives_the_panel_being_reopened() {
    let (state, repo) = opened();
    let held = state
        .enqueue(repo, OperationKind::Commit, "Committing")
        .await;

    let second = Arc::clone(&state);
    let waiting = tokio::spawn(async move {
        second
            .enqueue(repo, OperationKind::Commit, "Committing")
            .await
            .finish(true);
    });
    until(&state, 2).await;

    let snapshot = state.operations();
    assert_eq!(snapshot[0].phase, OperationPhase::Running);
    assert_eq!(snapshot[1].phase, OperationPhase::Queued);
    assert!(
        snapshot[0].id < snapshot[1].id,
        "running first, then the line behind it"
    );

    held.finish(true);
    waiting.await.unwrap();
}

#[tokio::test]
async fn every_phase_is_announced() {
    let (state, repo) = opened();
    let mut events = state.subscribe();

    let held = state.enqueue(repo, OperationKind::Pull, "Pulling").await;
    let second = Arc::clone(&state);
    let waiting = tokio::spawn(async move {
        second
            .enqueue(repo, OperationKind::Pull, "Pulling")
            .await
            .finish(false);
    });
    until(&state, 2).await;
    held.finish(true);
    waiting.await.unwrap();

    let mut phases = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let AppEvent::Operation(operation) = event {
            phases.push((operation.id, operation.phase, operation.success));
        }
    }

    assert_eq!(
        phases,
        vec![
            (0, OperationPhase::Running, None),
            (1, OperationPhase::Queued, None),
            (0, OperationPhase::Done, Some(true)),
            (1, OperationPhase::Running, None),
            (1, OperationPhase::Done, Some(false)),
        ]
    );
}
