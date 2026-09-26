// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::AppState;
use git_engine::Head;

#[test]
fn opening_a_repository_registers_it() {
    let f = test_fixtures::linear(2).unwrap();
    let state = AppState::new();
    let summary = state.open_repository(f.path()).unwrap();

    assert_eq!(state.list().len(), 1);
    assert!(state.get(summary.repo).is_some());
}

#[test]
fn the_summary_carries_head_and_branches() {
    let f = test_fixtures::branched().unwrap();
    let state = AppState::new();
    let summary = state.open_repository(f.path()).unwrap();

    match summary.head {
        Head::Branch { ref name, .. } => assert_eq!(name, "main"),
        ref other => panic!("expected a branch, got {other:?}"),
    }
    let names: Vec<&str> = summary.branches.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, vec!["dev", "main"]);
}

#[test]
fn the_summary_names_the_repository_after_its_folder() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let summary = state.open_repository(f.path()).unwrap();

    let folder = f.path().file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(summary.name, folder);
}

#[test]
fn opening_the_same_repository_twice_reuses_one_entry() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let first = state.open_repository(f.path()).unwrap();
    let second = state.open_repository(f.path()).unwrap();

    assert_eq!(first.repo, second.repo);
    assert_eq!(state.list().len(), 1);
}

#[test]
fn opening_from_a_subdirectory_resolves_to_the_same_repository() {
    let f = test_fixtures::unicode_paths().unwrap();
    let state = AppState::new();
    let root = state.open_repository(f.path()).unwrap();
    let nested = state.open_repository(&f.path().join("каталог")).unwrap();

    assert_eq!(
        root.repo, nested.repo,
        "discovery must collapse onto one entry"
    );
    assert_eq!(state.list().len(), 1);
}

#[test]
fn a_failed_open_registers_nothing() {
    let outside = tempfile::tempdir().unwrap();
    let dir = outside.path().join("not-a-repo");
    std::fs::create_dir_all(&dir).unwrap();
    let state = AppState::new();

    assert!(state.open_repository(&dir).is_err());
    assert!(state.list().is_empty(), "a failed open must leave no trace");
}

#[test]
fn opening_announces_the_repository_on_the_event_bus() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let mut events = state.subscribe();

    let summary = state.open_repository(f.path()).unwrap();

    match events.try_recv() {
        Ok(app_state::AppEvent::RepoOpened { repo }) => assert_eq!(repo, summary.repo),
        other => panic!("expected RepoOpened, got {other:?}"),
    }
}

#[test]
fn an_empty_repository_opens_without_error() {
    // INV-07: a freshly created repository is normal, not a failure.
    let f = test_fixtures::empty().unwrap();
    let state = AppState::new();
    let summary = state.open_repository(f.path()).unwrap();

    assert!(matches!(summary.head, Head::Unborn { .. }));
    assert!(summary.branches.is_empty());
}

#[test]
fn the_summary_carries_the_tag_separator_and_rereads_it_on_every_open() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    assert_eq!(
        state.open_repository(f.path()).unwrap().tag_group_separator,
        "/"
    );

    f.git(&["config", "cogit.tagGroupSeparator", "-"]).unwrap();
    assert_eq!(
        state.open_repository(f.path()).unwrap().tag_group_separator,
        "-"
    );
}

#[test]
fn ref_dates_cover_branches_and_tags() {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["tag", "v1"]).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let mut names: Vec<String> = state
        .ref_dates(repo)
        .unwrap()
        .into_iter()
        .map(|entry| entry.full_name)
        .collect();
    names.sort();
    assert_eq!(names, vec!["refs/heads/main", "refs/tags/v1"]);
}

// Open and a folder dropped on the window at the same moment both found nothing registered
// and each registered the path: two ids, two watchers, two queues for one repository.
#[test]
fn one_path_opened_twice_at_once_is_one_repository() {
    let f = test_fixtures::linear(3).unwrap();
    let state = std::sync::Arc::new(AppState::new());
    let start = std::sync::Arc::new(std::sync::Barrier::new(2));

    let opens: Vec<_> = (0..2)
        .map(|_| {
            let state = std::sync::Arc::clone(&state);
            let start = std::sync::Arc::clone(&start);
            let path = f.path().to_path_buf();
            std::thread::spawn(move || {
                start.wait();
                state.open_repository(&path).unwrap().repo
            })
        })
        .collect();
    let ids: Vec<_> = opens.into_iter().map(|open| open.join().unwrap()).collect();

    assert_eq!(ids[0], ids[1]);
    assert_eq!(state.list().len(), 1);
}
