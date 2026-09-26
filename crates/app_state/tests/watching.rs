//! Only the repository the panels show is watched (R-351). A watch holds a handle open on
//! every folder it covers, and Windows refuses to rename a folder with a handle open
//! anywhere below it — so a watcher left on a repository nobody looks at kept its folder,
//! and every folder above it, from being renamed or moved.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppEvent, AppState, DEFAULT_CHUNK_SIZE, RepoId};
use git_engine::CommitQuery;
use std::time::{Duration, Instant};
use tokio::sync::broadcast::Receiver;

/// Past the debounce and the quiet window a watcher starts with.
const SETTLE: Duration = Duration::from_millis(1500);

fn heard(events: &mut Receiver<AppEvent>, repo: RepoId, within: Duration) -> bool {
    let deadline = Instant::now() + within;
    loop {
        match events.try_recv() {
            Ok(AppEvent::RepoChanged { repo: from, .. }) if from == repo => return true,
            Ok(_) => {}
            Err(_) if Instant::now() > deadline => return false,
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

fn graph_total(state: &AppState, repo: RepoId) -> u32 {
    let generation = state.begin_graph();
    state
        .build_graph(
            repo,
            &CommitQuery::default(),
            generation,
            DEFAULT_CHUNK_SIZE,
            |_| true,
        )
        .unwrap();
    state.graph_window(repo, generation, 0, 0).unwrap().total
}

#[test]
fn opening_a_repository_is_not_showing_it() {
    let a = test_fixtures::linear(2).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(a.path()).unwrap().repo;
    let mut events = state.subscribe();

    std::fs::write(a.path().join("file0.txt"), "opened, not shown\n").unwrap();
    assert!(
        !heard(&mut events, repo, SETTLE),
        "restoring a session opens every repository; only the one shown is watched"
    );

    state.show_repository(Some(repo));
    std::fs::write(a.path().join("file0.txt"), "shown now\n").unwrap();
    assert!(heard(&mut events, repo, Duration::from_secs(5)));
}

#[test]
fn a_repository_left_for_another_is_not_watched() {
    let a = test_fixtures::linear(2).unwrap();
    let b = test_fixtures::branched().unwrap();
    let state = AppState::new();
    let first = state.open_repository(a.path()).unwrap().repo;
    let second = state.open_repository(b.path()).unwrap().repo;
    state.show_repository(Some(first));
    state.show_repository(Some(second));
    let mut events = state.subscribe();

    std::fs::write(a.path().join("file0.txt"), "changed while b is shown\n").unwrap();
    assert!(!heard(&mut events, first, SETTLE));

    std::fs::write(b.path().join("new.txt"), "b is shown\n").unwrap();
    assert!(heard(&mut events, second, Duration::from_secs(5)));
    assert!(state.get(first).is_some(), "left, not closed");
}

#[test]
fn the_folder_above_a_repository_left_for_another_can_be_renamed() {
    let source = test_fixtures::linear(2).unwrap();
    let other = test_fixtures::linear(1).unwrap();
    let outer = tempfile::tempdir().unwrap();
    let above = outer.path().join("projects");
    let root = above.join("repo");
    source
        .git(&[
            "clone",
            "-q",
            &source.path().to_string_lossy(),
            &root.to_string_lossy(),
        ])
        .unwrap();
    let state = AppState::new();
    let repo = state.open_repository(&root).unwrap().repo;
    let elsewhere = state.open_repository(other.path()).unwrap().repo;
    let moved = outer.path().join("renamed");

    state.show_repository(Some(repo));
    #[cfg(windows)]
    {
        let held = std::fs::rename(&above, &moved);
        if held.is_ok() {
            std::fs::rename(&moved, &above).unwrap();
        }
        assert!(
            held.is_err(),
            "the check proves nothing if a watch holds no handle"
        );
    }

    state.show_repository(Some(elsewhere));
    let renamed = std::fs::rename(&above, &moved);
    if renamed.is_ok() {
        std::fs::rename(&moved, &above).unwrap();
    }
    renamed.unwrap();
}

#[test]
fn coming_back_sees_what_changed_while_it_was_not_watched() {
    let a = test_fixtures::linear(2).unwrap();
    let b = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(a.path()).unwrap().repo;
    let other = state.open_repository(b.path()).unwrap().repo;
    state.show_repository(Some(repo));
    let before = graph_total(&state, repo);
    assert!(
        !state
            .overviews()
            .iter()
            .any(|row| row.repo == repo && row.dirty)
    );

    state.show_repository(Some(other));
    let oid = a
        .commit_file(10, "while-away.txt", "from a terminal\n")
        .unwrap();
    std::fs::write(a.path().join("file0.txt"), "edited while away\n").unwrap();
    state.show_repository(Some(repo));

    let row = state
        .overviews()
        .into_iter()
        .find(|row| row.repo == repo)
        .unwrap();
    assert!(
        row.dirty,
        "the row read before leaving is not the answer any more"
    );
    let summary = state.open_repository(a.path()).unwrap();
    assert!(matches!(summary.head, git_engine::Head::Branch { oid: head, .. } if head == oid));
    assert_eq!(
        graph_total(&state, repo),
        before + 1,
        "the moved ref retires the cached graph"
    );

    let mut events = state.subscribe();
    std::fs::write(a.path().join("file1.txt"), "watched again\n").unwrap();
    assert!(heard(&mut events, repo, Duration::from_secs(5)));
}

// With no watcher nothing tells the row of a repository left behind that it went stale;
// its pulse, read from the disk when the window regains focus, does.
#[test]
fn a_pulse_that_contradicts_the_row_of_a_repository_not_watched_retires_it() {
    let a = test_fixtures::linear(2).unwrap();
    let b = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(a.path()).unwrap().repo;
    let other = state.open_repository(b.path()).unwrap().repo;
    state.show_repository(Some(other));
    let branch = |state: &AppState| {
        state
            .overviews()
            .into_iter()
            .find(|row| row.repo == repo)
            .and_then(|row| row.branch)
    };
    assert_eq!(branch(&state).as_deref(), Some("main"));

    a.git(&["checkout", "-q", "-b", "elsewhere"]).unwrap();
    let pulse = state.pulse(a.path());

    assert_eq!(pulse.branch.as_deref(), Some("elsewhere"));
    assert_eq!(branch(&state).as_deref(), Some("elsewhere"));
    let read = state.rows_read();
    let _ = state.pulse(a.path());
    let _ = state.overviews();
    assert_eq!(state.rows_read(), read, "a pulse that agrees keeps the row");
}
