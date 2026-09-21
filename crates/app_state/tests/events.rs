// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppEvent, AppState};

fn drain(rx: &mut tokio::sync::broadcast::Receiver<AppEvent>) -> Vec<AppEvent> {
    let mut seen = Vec::new();
    while let Ok(event) = rx.try_recv() {
        seen.push(event);
    }
    seen
}

#[tokio::test]
async fn an_operation_announces_its_start_and_its_end() {
    let state = AppState::new();
    let mut rx = state.subscribe();

    let value = state.tracked("Fetching", || Ok::<_, git_engine::GitError>(7));

    assert_eq!(value.unwrap(), 7);
    let seen = drain(&mut rx);
    assert!(
        matches!(seen[0], AppEvent::OperationStarted { ref label, .. } if label == "Fetching"),
        "{seen:?}"
    );
    assert!(
        matches!(seen[1], AppEvent::OperationFinished { success: true, .. }),
        "{seen:?}"
    );
}

#[tokio::test]
async fn a_failed_operation_still_announces_its_end() {
    let state = AppState::new();
    let mut rx = state.subscribe();

    let result = state.tracked("Pushing", || {
        Err::<(), _>(git_engine::GitError::InvalidState("no".to_owned()))
    });

    assert!(result.is_err());
    let seen = drain(&mut rx);
    assert!(
        matches!(seen[1], AppEvent::OperationFinished { success: false, .. }),
        "{seen:?}"
    );
}

#[tokio::test]
async fn start_and_finish_carry_the_same_id() {
    let state = AppState::new();
    let mut rx = state.subscribe();

    state
        .tracked("Fetching", || Ok::<_, git_engine::GitError>(()))
        .unwrap();

    let seen = drain(&mut rx);
    let started = match seen[0] {
        AppEvent::OperationStarted { id, .. } => id,
        ref other => panic!("{other:?}"),
    };
    let finished = match seen[1] {
        AppEvent::OperationFinished { id, .. } => id,
        ref other => panic!("{other:?}"),
    };
    assert_eq!(started, finished);
}

#[tokio::test]
async fn two_operations_do_not_share_an_id() {
    let state = AppState::new();
    let mut rx = state.subscribe();

    state
        .tracked("One", || Ok::<_, git_engine::GitError>(()))
        .unwrap();
    state
        .tracked("Two", || Ok::<_, git_engine::GitError>(()))
        .unwrap();

    let ids: Vec<u32> = drain(&mut rx)
        .into_iter()
        .filter_map(|event| match event {
            AppEvent::OperationStarted { id, .. } => Some(id),
            _ => None,
        })
        .collect();
    assert_ne!(ids[0], ids[1]);
}

#[tokio::test]
async fn a_subscriber_that_falls_behind_recovers_instead_of_dying() {
    let state = AppState::new();
    let mut slow = state.subscribe();

    for _ in 0..600 {
        state.emit(AppEvent::RepoOpened {
            repo: app_state::RepoId(1),
        });
    }

    assert!(matches!(
        slow.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_))
    ));
    assert!(
        slow.try_recv().is_ok(),
        "the bus keeps delivering after a lag"
    );
}

#[tokio::test]
async fn one_slow_subscriber_does_not_stop_another() {
    let state = AppState::new();
    let mut slow = state.subscribe();
    let mut fast = state.subscribe();

    state.emit(AppEvent::RepoOpened {
        repo: app_state::RepoId(1),
    });
    assert!(fast.try_recv().is_ok());

    for _ in 0..600 {
        state.emit(AppEvent::RepoClosed {
            repo: app_state::RepoId(1),
        });
    }

    let _ = slow.try_recv();
    let _ = fast.try_recv();
    assert!(fast.try_recv().is_ok());
    assert!(slow.try_recv().is_ok());
}

#[tokio::test]
async fn every_git_command_announces_itself_without_carrying_its_output() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let mut rx = state.subscribe();

    state.create_branch(repo, "topic", None, false).unwrap();

    let notices: Vec<_> = drain(&mut rx)
        .into_iter()
        .filter_map(|event| match event {
            AppEvent::CommandRecorded(notice) => Some(notice),
            _ => None,
        })
        .collect();
    let notice = notices.first().expect("a command must announce itself");
    assert_eq!(notice.operation, "Branch");
    assert!(state.command_outcome(notice.id).is_some());
}

#[tokio::test]
async fn a_failure_is_announced_as_one() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let mut rx = state.subscribe();

    let _ = state.stage_paths(repo, &["never-existed.txt".to_owned()]);

    let severities: Vec<_> = drain(&mut rx)
        .into_iter()
        .filter_map(|event| match event {
            AppEvent::CommandRecorded(notice) => Some(notice.severity),
            _ => None,
        })
        .collect();
    assert!(
        severities.contains(&git_engine::Severity::Failure),
        "{severities:?}"
    );
}
