// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::AppState;
use git_engine::{GitError, ModuleProblem};

#[test]
fn several_repositories_can_be_open_at_once() {
    let a = test_fixtures::linear(2).unwrap();
    let b = test_fixtures::branched().unwrap();
    let state = AppState::new();

    let first = state.open_repository(a.path()).unwrap().repo;
    let second = state.open_repository(b.path()).unwrap().repo;

    assert_ne!(first, second);
    assert_eq!(state.overviews().len(), 2);
}

#[test]
fn opening_the_same_path_twice_reuses_the_entry() {
    let f = test_fixtures::linear(2).unwrap();
    let state = AppState::new();

    let first = state.open_repository(f.path()).unwrap().repo;
    let again = state.open_repository(f.path()).unwrap().repo;

    assert_eq!(first, again);
    assert_eq!(state.overviews().len(), 1);
}

#[test]
fn an_overview_carries_what_the_tree_row_shows() {
    let f = test_fixtures::with_remote().unwrap();
    std::fs::write(f.path().join("file0.txt"), "dirty\n").unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let overview = state
        .overviews()
        .into_iter()
        .find(|o| o.repo == repo)
        .unwrap();

    assert_eq!(overview.branch.as_deref(), Some("main"));
    assert_eq!(overview.ahead, 2);
    assert_eq!(overview.behind, 1);
    assert!(overview.dirty);
}

#[test]
fn a_clean_repository_is_not_reported_as_dirty() {
    let f = test_fixtures::linear(2).unwrap();
    let state = AppState::new();
    state.open_repository(f.path()).unwrap();

    assert!(!state.overviews()[0].dirty);
}

#[test]
fn closing_a_repository_removes_it_from_the_list() {
    let a = test_fixtures::linear(2).unwrap();
    let b = test_fixtures::branched().unwrap();
    let state = AppState::new();
    let first = state.open_repository(a.path()).unwrap().repo;
    state.open_repository(b.path()).unwrap();

    assert!(state.close_repository(first));

    assert_eq!(state.overviews().len(), 1);
    assert!(state.overviews().iter().all(|o| o.repo != first));
}

#[test]
fn closing_a_repository_leaves_the_files_on_disk() {
    let f = test_fixtures::linear(2).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    state.close_repository(repo);

    assert!(f.path().join("file0.txt").exists());
}

#[test]
fn closing_something_that_is_not_open_is_not_an_error() {
    let state = AppState::new();
    assert!(!state.close_repository(app_state::RepoId(9)));
}

#[test]
fn closing_stops_watching_it() {
    let f = test_fixtures::linear(2).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    state.show_repository(Some(repo));
    let mut events = state.subscribe();
    // The control: a watcher that never started would keep quiet after closing too.
    std::fs::write(f.path().join("file0.txt"), "changed while open\n").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let heard = loop {
        match events.try_recv() {
            Ok(app_state::AppEvent::RepoChanged { .. }) => break true,
            Ok(_) => {}
            Err(_) if std::time::Instant::now() > deadline => break false,
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(20)),
        }
    };
    assert!(heard, "an open repository must be watched");
    state.close_repository(repo);
    while events.try_recv().is_ok() {}

    std::fs::write(f.path().join("file0.txt"), "changed after closing\n").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(600));

    assert!(
        events.try_recv().is_err(),
        "a closed repository must not keep an OS watch alive"
    );
}

#[test]
fn overviews_are_sorted_by_name() {
    let names = [
        test_fixtures::linear(1).unwrap(),
        test_fixtures::linear(2).unwrap(),
    ];
    let state = AppState::new();
    for f in &names {
        state.open_repository(f.path()).unwrap();
    }

    let listed: Vec<String> = state.overviews().into_iter().map(|o| o.name).collect();
    let mut sorted = listed.clone();
    sorted.sort();
    assert_eq!(listed, sorted);
}

/// The tree row says `<merging>` or `<detached>` beside the name (#22).
#[test]
fn an_overview_says_what_the_repository_is_in_the_middle_of() {
    let f = test_fixtures::conflicted().unwrap();
    let state = AppState::new();
    state.open_repository(f.path()).unwrap();

    assert_eq!(state.overviews()[0].state, git_engine::RepoState::Merging);
}

#[test]
fn an_overview_says_when_head_is_detached() {
    let f = test_fixtures::detached_head().unwrap();
    let state = AppState::new();
    state.open_repository(f.path()).unwrap();

    assert!(matches!(
        state.overviews()[0].state,
        git_engine::RepoState::DetachedHead { .. }
    ));
}

#[test]
fn an_overview_of_an_empty_repository_has_no_branch_yet() {
    let f = test_fixtures::empty().unwrap();
    let state = AppState::new();
    state.open_repository(f.path()).unwrap();

    assert!(state.overviews()[0].branch.is_none());
}

#[test]
fn a_repository_still_on_disk_is_not_reported_missing() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    state.open_repository(f.path()).unwrap();

    assert!(state.overviews().iter().all(|row| !row.missing));
}

#[test]
fn a_repository_that_left_the_disk_is_reported_missing() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    state.open_repository(f.path()).unwrap();

    // The folder is gone, but the user's list should still show the row — with a mark, so
    // they can remove it deliberately rather than wonder where it went (T3.7).
    std::fs::remove_dir_all(f.path().join(".git")).unwrap();

    let rows = state.overviews();
    assert_eq!(rows.len(), 1, "the row must survive to be removable");
    assert!(rows[0].missing);
}

#[test]
fn a_missing_repository_reports_nothing_it_cannot_know() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    state.open_repository(f.path()).unwrap();
    std::fs::remove_dir_all(f.path().join(".git")).unwrap();

    let row = state.overviews().into_iter().next().unwrap();
    assert_eq!(row.branch, None);
    assert!(!row.dirty);
    assert_eq!((row.ahead, row.behind), (0, 0));
    assert_eq!(row.state, git_engine::RepoState::Clean);
}

// --- a submodule opened from the tree is not a repository in the list -----------------

/// SmartGit opens a submodule by double-clicking its node: the panels follow it, but the
/// list of repositories does not grow a second entry for it (doc/12-risks.md, R-109).
#[test]
fn opening_a_submodule_does_not_add_it_to_the_list() {
    let f = test_fixtures::with_submodule().unwrap();
    let state = AppState::new();
    let parent = state.open_repository(f.path()).unwrap().repo;

    let child = state.open_submodule(parent, "vendor/lib").unwrap();

    assert_ne!(child.repo, parent);
    assert_eq!(state.overviews().len(), 1, "only the parent is listed");
}

#[test]
fn a_submodule_opened_from_the_tree_is_still_a_repository_to_work_in() {
    let f = test_fixtures::with_submodule().unwrap();
    let state = AppState::new();
    let parent = state.open_repository(f.path()).unwrap().repo;

    let child = state.open_submodule(parent, "vendor/lib").unwrap();

    assert!(state.repo_status(child.repo).is_ok());
}

/// Requirement 6.10: the same path can be an open submodule and an entry of its own, and
/// asking for it by hand is what makes it an entry.
#[test]
fn opening_the_same_path_by_hand_afterwards_does_list_it() {
    let f = test_fixtures::with_submodule().unwrap();
    let state = AppState::new();
    let parent = state.open_repository(f.path()).unwrap().repo;
    state.open_submodule(parent, "vendor/lib").unwrap();

    state.open_repository(&f.path().join("vendor/lib")).unwrap();

    assert_eq!(state.overviews().len(), 2);
}

/// And the other way round: a submodule of a repository already open by hand must not
/// disappear from the list when its node is double-clicked.
#[test]
fn opening_a_listed_repository_as_a_submodule_leaves_it_listed() {
    let f = test_fixtures::with_submodule().unwrap();
    let state = AppState::new();
    let parent = state.open_repository(f.path()).unwrap().repo;
    state.open_repository(&f.path().join("vendor/lib")).unwrap();

    state.open_submodule(parent, "vendor/lib").unwrap();

    assert_eq!(state.overviews().len(), 2);
}

// Every submodule visited from the tree stayed registered, its handle held, until exit.
#[test]
fn closing_a_repository_closes_the_submodules_opened_from_it() {
    let f = test_fixtures::with_nested_submodule().unwrap();
    let state = AppState::new();
    let parent = state.open_repository(f.path()).unwrap().repo;
    let middle = state.open_submodule(parent, "vendor/middle").unwrap().repo;
    let deep = state.open_submodule(middle, "deep/inner").unwrap().repo;
    assert!(state.repo_status(deep).is_ok());

    state.close_repository(parent);

    for gone in [middle, deep] {
        assert!(
            state.repo_status(gone).is_err(),
            "a submodule goes with the repository it was opened from"
        );
    }
    assert_eq!(state.repositories_held(), 0);
}

#[test]
fn the_submodule_on_screen_stays_open_when_its_parent_closes() {
    let f = test_fixtures::with_submodule().unwrap();
    let state = AppState::new();
    let parent = state.open_repository(f.path()).unwrap().repo;
    let child = state.open_submodule(parent, "vendor/lib").unwrap().repo;
    state.show_repository(Some(child));

    state.close_repository(parent);

    assert!(state.repo_status(child).is_ok(), "the panels still show it");
}

// A Blame window left open, its repository closed in the main window: F5 said "Not a Git
// repository: id 3" (BE-013).
#[test]
fn a_call_on_a_closed_repository_says_it_was_closed() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    state.close_repository(repo);

    let refused = state.repo_status(repo).unwrap_err();

    assert!(
        matches!(&refused, GitError::InvalidState(text) if text.contains("closed in Cogit")),
        "{refused:?}"
    );
}

#[test]
fn a_submodule_opened_by_hand_as_well_outlives_its_parent() {
    let f = test_fixtures::with_submodule().unwrap();
    let state = AppState::new();
    let parent = state.open_repository(f.path()).unwrap().repo;
    let child = state.open_submodule(parent, "vendor/lib").unwrap().repo;
    state.open_repository(&f.path().join("vendor/lib")).unwrap();

    state.close_repository(parent);

    assert!(
        state.repo_status(child).is_ok(),
        "it is an entry of its own"
    );
}

// --- one resolution for listing and opening (doc/12-risks.md, R-149) --------------------

/// The reported case: after one submodule was opened, every other one failed, because the
/// path was built from the submodule on screen instead of the repository owning the tree.
#[test]
fn a_second_submodule_opens_after_the_first_one_did() {
    let f = test_fixtures::with_nested_submodule().unwrap();
    let state = AppState::new();
    let parent = state.open_repository(f.path()).unwrap().repo;

    state.open_submodule(parent, "vendor/middle").unwrap();
    let deep = state
        .open_submodule(parent, "vendor/middle/deep/inner")
        .unwrap();

    assert!(
        deep.root
            .replace('\\', "/")
            .ends_with("vendor/middle/deep/inner")
    );
}

#[test]
fn a_node_is_opened_where_the_tree_listed_it() {
    let f = test_fixtures::with_nested_submodule().unwrap();
    let state = AppState::new();
    let parent = state.open_repository(f.path()).unwrap().repo;

    let listed = state.submodules_under(parent, "vendor/middle").unwrap();
    let key = format!("vendor/middle/{}", listed[0].path);

    assert!(state.open_submodule(parent, &key).is_ok());
}

/// Discovery from an empty submodule directory lands in the parent; the tree must get a
/// reason, not the parent's own submodules dressed up as the child's.
#[test]
fn an_uninitialised_submodule_lists_nothing_of_its_parent() {
    let f = test_fixtures::with_submodule().unwrap();
    let state = AppState::new();
    let parent = state.open_repository(f.path()).unwrap().repo;
    let path = f.path().join("vendor/lib");
    std::fs::remove_dir_all(&path).unwrap();
    std::fs::create_dir_all(&path).unwrap();

    match state.submodules_under(parent, "vendor/lib") {
        Err(GitError::ModuleUnavailable(ModuleProblem::NotInitialised { .. })) => {}
        other => panic!("expected NotInitialised, got {other:?}"),
    }
}

#[test]
fn opening_an_uninitialised_submodule_names_the_reason() {
    let f = test_fixtures::with_submodule().unwrap();
    let state = AppState::new();
    let parent = state.open_repository(f.path()).unwrap().repo;
    let path = f.path().join("vendor/lib");
    std::fs::remove_dir_all(&path).unwrap();
    std::fs::create_dir_all(&path).unwrap();

    assert!(matches!(
        state.open_submodule(parent, "vendor/lib"),
        Err(GitError::ModuleUnavailable(
            ModuleProblem::NotInitialised { .. }
        ))
    ));
}

// --- health checks run for the repository in the list (doc/12-risks.md, R-150) ---------

#[test]
fn the_health_of_an_open_repository_can_be_asked_for() {
    let f = test_fixtures::with_submodule().unwrap();
    let state = AppState::new();
    let parent = state.open_repository(f.path()).unwrap().repo;
    let module = f.path().join("vendor/lib");
    std::fs::remove_dir_all(&module).unwrap();
    std::fs::create_dir_all(&module).unwrap();
    std::fs::write(module.join(".git"), "gitdir: ../../nowhere\n").unwrap();

    let report = state.health(parent).unwrap();
    assert!(report.iter().any(|finding| finding.module == "vendor/lib"));
}

// --- Repository ▸ Edit Git Config (doc/12-risks.md, R-155) -------------------------------

#[test]
fn a_repositorys_config_is_read_and_written_through_the_state() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let file = state
        .config_file(Some(repo), git_engine::ConfigScope::Repository)
        .unwrap();
    assert!(file.text.contains("[core]"));

    let edited = format!("{}[cogit]\n\tprobe = yes\n", file.text);
    state
        .save_config_file(
            Some(repo),
            git_engine::ConfigScope::Repository,
            &edited,
            file.crlf,
        )
        .unwrap();
    assert!(
        std::fs::read_to_string(f.git_dir().join("config"))
            .unwrap()
            .contains("probe = yes")
    );
}

#[test]
fn the_repository_scope_needs_a_repository() {
    let state = AppState::new();
    assert!(
        state
            .config_file(None, git_engine::ConfigScope::Repository)
            .is_err()
    );
}
