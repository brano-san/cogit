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

#[test]
fn the_session_may_end_when_nothing_is_queued() {
    let (state, _) = opened();
    assert_eq!(state.session_end_blocker(), None);
}

#[tokio::test]
async fn the_session_end_is_held_while_a_push_runs() {
    let (state, repo) = opened();
    let held = state.enqueue(repo, OperationKind::Push, "Pushing").await;

    assert_eq!(
        state.session_end_blocker().as_deref(),
        Some("1 operation is still running")
    );

    held.finish(true);
    assert_eq!(state.session_end_blocker(), None);
}

#[tokio::test]
async fn the_session_end_counts_what_waits_behind_it() {
    let (state, repo) = opened();
    let held = state.enqueue(repo, OperationKind::Push, "Pushing").await;
    let second = Arc::clone(&state);
    let waiting = tokio::spawn(async move {
        second
            .enqueue(repo, OperationKind::Fetch, "Fetching")
            .await
            .finish(true);
    });
    until(&state, 2).await;

    assert_eq!(
        state.session_end_blocker().as_deref(),
        Some("2 operations are still running")
    );

    held.finish(true);
    waiting.await.unwrap();
}

#[tokio::test]
async fn an_empty_queue_is_idle_at_once() {
    let (state, _) = opened();
    tokio::time::timeout(
        test_fixtures::scaled(Duration::from_secs(1)),
        state.until_idle(),
    )
    .await
    .unwrap();
}

// The close watchdog exited two seconds after a silent page with a rebase running, and
// the exit killed git half-way: index.lock left behind, a rebase half done.
#[tokio::test]
async fn idle_waits_for_the_operation_running_and_the_one_behind_it() {
    let (state, repo) = opened();
    let held = state.enqueue(repo, OperationKind::Rebase, "Rebasing").await;
    let second = Arc::clone(&state);
    let (go, wait) = tokio::sync::oneshot::channel::<()>();
    let behind = tokio::spawn(async move {
        let permit = second
            .enqueue(repo, OperationKind::Commit, "Committing")
            .await;
        let _ = wait.await;
        permit.finish(true);
    });
    until(&state, 2).await;
    let waiter = Arc::clone(&state);
    let idle = tokio::spawn(async move { waiter.until_idle().await });

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!idle.is_finished(), "two operations are still in the queue");
    held.finish(true);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !idle.is_finished(),
        "the commit behind the rebase is running now"
    );

    go.send(()).unwrap();
    behind.await.unwrap();
    tokio::time::timeout(test_fixtures::scaled(Duration::from_secs(5)), idle)
        .await
        .unwrap()
        .unwrap();
}

async fn waits_behind(state: &Arc<AppState>, running: RepoId, next: RepoId) {
    let held = state.enqueue(running, OperationKind::Push, "Pushing").await;
    let second = Arc::clone(state);
    let waiting = tokio::spawn(async move {
        second
            .enqueue(next, OperationKind::Worktree, "Removing worktree")
            .await
            .finish(true);
    });
    until(state, 2).await;
    assert!(
        !waiting.is_finished(),
        "two git processes at once in one repository's files"
    );
    held.finish(true);
    waiting.await.unwrap();
}

// A worktree or submodule opened from the tree had a lane of its own: the owner's Remove
// --force or Update Submodule ran in its folder beside its own push or commit (CC-007).
#[tokio::test]
async fn a_worktree_opened_from_its_owner_waits_in_the_owners_lane() {
    let f = test_fixtures::with_worktree().unwrap();
    let state = Arc::new(AppState::new());
    let owner = state.open_repository(f.path()).unwrap().repo;
    let path = state
        .worktrees(owner)
        .unwrap()
        .into_iter()
        .find(|entry| !entry.is_main)
        .unwrap()
        .path;
    let linked = state.open_worktree(owner, &path).unwrap().repo;

    waits_behind(&state, linked, owner).await;
    waits_behind(&state, owner, linked).await;
}

#[tokio::test]
async fn a_linked_worktree_opened_by_hand_shares_its_repository_lane() {
    let f = test_fixtures::with_worktree().unwrap();
    let state = Arc::new(AppState::new());
    let owner = state.open_repository(f.path()).unwrap().repo;
    let path = state
        .worktrees(owner)
        .unwrap()
        .into_iter()
        .find(|entry| !entry.is_main)
        .unwrap()
        .path;
    let linked = state
        .open_repository(std::path::Path::new(&path))
        .unwrap()
        .repo;

    waits_behind(&state, linked, owner).await;
}

#[tokio::test]
async fn a_submodule_opened_from_its_parent_waits_in_the_parents_lane() {
    let f = test_fixtures::with_submodule().unwrap();
    let state = Arc::new(AppState::new());
    let parent = state.open_repository(f.path()).unwrap().repo;
    let child = state.open_submodule(parent, "vendor/lib").unwrap().repo;

    waits_behind(&state, child, parent).await;
}

#[tokio::test]
async fn a_background_fetch_waits_its_turn_and_holds_the_lane() {
    let (state, repo) = opened();
    let push = state.enqueue(repo, OperationKind::Push, "Pushing").await;

    let background = Arc::clone(&state);
    let (took, turn) = std::sync::mpsc::channel();
    let (done, finished) = std::sync::mpsc::channel::<()>();
    let fetch = tokio::task::spawn_blocking(move || {
        let _turn = background.background_turn(repo);
        took.send(()).unwrap();
        finished.recv().unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(turn.try_recv().is_err(), "it ran beside the push");
    assert_eq!(state.operations().len(), 1, "nobody waits on it yet");
    assert_eq!(
        state.session_end_blocker().as_deref(),
        Some("2 operations are still running")
    );

    push.finish(true);
    tokio::task::spawn_blocking(move || turn.recv().unwrap())
        .await
        .unwrap();

    let commit = Arc::clone(&state);
    let committing = tokio::spawn(async move {
        commit
            .enqueue(repo, OperationKind::Commit, "Committing")
            .await
            .finish(true);
    });
    until(&state, 2).await;
    let shown = state.operations();
    assert_eq!(shown[0].kind, OperationKind::Fetch);
    assert_eq!(shown[0].phase, OperationPhase::Running);
    assert_eq!(shown[1].phase, OperationPhase::Queued);

    done.send(()).unwrap();
    fetch.await.unwrap();
    committing.await.unwrap();
    until(&state, 0).await;
}

// A Commit sat "Queued" behind a background fetch with nothing shown running.
#[tokio::test]
async fn a_background_fetch_is_announced_once_someone_waits_behind_it() {
    let (state, repo) = opened();
    let mut events = state.subscribe();
    let background = Arc::clone(&state);
    let (took, turn) = std::sync::mpsc::channel();
    let (done, finished) = std::sync::mpsc::channel::<()>();
    let fetch = tokio::task::spawn_blocking(move || {
        let _turn = background.background_turn(repo);
        took.send(()).unwrap();
        finished.recv().unwrap();
    });
    tokio::task::spawn_blocking(move || turn.recv().unwrap())
        .await
        .unwrap();
    assert!(state.operations().is_empty(), "alone, it stays quiet");

    let commit = Arc::clone(&state);
    let committing = tokio::spawn(async move {
        commit
            .enqueue(repo, OperationKind::Commit, "Committing")
            .await
            .finish(true);
    });
    until(&state, 2).await;
    done.send(()).unwrap();
    fetch.await.unwrap();
    committing.await.unwrap();

    let mut phases = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let AppEvent::Operation(operation) = event {
            phases.push((operation.kind, operation.phase, operation.cancellable));
        }
    }
    // The commit's `Running` and the fetch's `Done` come from two threads, in either order.
    assert_eq!(
        phases[..2],
        [
            (OperationKind::Fetch, OperationPhase::Running, true),
            (OperationKind::Commit, OperationPhase::Queued, false),
        ]
    );
    assert!(phases.contains(&(OperationKind::Fetch, OperationPhase::Done, true)));
    assert_eq!(
        phases.last(),
        Some(&(OperationKind::Commit, OperationPhase::Done, false))
    );
    assert_eq!(phases.len(), 5, "{phases:?}");
}

#[tokio::test]
async fn idle_and_the_session_end_wait_for_a_background_fetch() {
    let (state, repo) = opened();
    let background = Arc::clone(&state);
    let (took, turn) = std::sync::mpsc::channel();
    let (done, finished) = std::sync::mpsc::channel::<()>();
    let fetch = tokio::task::spawn_blocking(move || {
        let _turn = background.background_turn(repo);
        took.send(()).unwrap();
        finished.recv().unwrap();
    });
    tokio::task::spawn_blocking(move || turn.recv().unwrap())
        .await
        .unwrap();

    assert_eq!(
        state.session_end_blocker().as_deref(),
        Some("1 operation is still running")
    );
    let waiter = Arc::clone(&state);
    let idle = tokio::spawn(async move { waiter.until_idle().await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!idle.is_finished(), "the fetch still writes refs");

    done.send(()).unwrap();
    fetch.await.unwrap();
    tokio::time::timeout(test_fixtures::scaled(Duration::from_secs(5)), idle)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.session_end_blocker(), None);
}
