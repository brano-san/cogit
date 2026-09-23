// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, RepoId};

fn open(f: &test_fixtures::Fixture) -> (AppState, RepoId) {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    (state, repo)
}

#[test]
fn nothing_to_undo_on_a_fresh_repository() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);

    assert!(state.safety_log().is_empty());
    assert!(state.undo_last(repo).is_err());
}

#[test]
fn discarding_a_file_is_undoable_and_restores_the_exact_content() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);
    std::fs::write(f.path().join("file0.txt"), "work in progress\n").unwrap();

    state
        .discard_paths(repo, &["file0.txt".to_owned()])
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(f.path().join("file0.txt")).unwrap(),
        "content 0\n"
    );

    state.undo_last(repo).unwrap();

    assert_eq!(
        std::fs::read_to_string(f.path().join("file0.txt")).unwrap(),
        "work in progress\n"
    );
}

#[test]
fn discarding_an_untracked_file_is_undoable() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);
    std::fs::write(f.path().join("scratch.txt"), "not added yet\n").unwrap();

    state
        .discard_paths(repo, &["scratch.txt".to_owned()])
        .unwrap();
    assert!(!f.path().join("scratch.txt").exists());

    state.undo_last(repo).unwrap();

    assert_eq!(
        std::fs::read_to_string(f.path().join("scratch.txt")).unwrap(),
        "not added yet\n"
    );
}

#[test]
fn deleting_a_branch_is_undoable_and_restores_the_same_oid() {
    let f = test_fixtures::branched().unwrap();
    let (state, repo) = open(&f);
    let before = f.oid("dev").unwrap();

    state.delete_branch(repo, "dev", true).unwrap();
    assert!(f.oid("dev").is_err());

    state.undo_last(repo).unwrap();

    assert_eq!(f.oid("dev").unwrap(), before);
}

#[test]
fn the_journal_describes_what_happened() {
    let f = test_fixtures::branched().unwrap();
    let (state, repo) = open(&f);

    state.delete_branch(repo, "dev", true).unwrap();

    let entry = state.safety_log().into_iter().next().unwrap();
    assert!(entry.description.contains("dev"), "{entry:?}");
    assert!(entry.undoable);
}

#[test]
fn an_operation_that_cannot_be_undone_says_so() {
    let f = test_fixtures::linear(3).unwrap();
    let (state, repo) = open(&f);

    state
        .checkout(
            repo,
            &git_engine::CheckoutTarget::Branch {
                name: "main".to_owned(),
            },
        )
        .unwrap();

    let entry = state.safety_log().into_iter().next().unwrap();
    assert!(
        !entry.undoable,
        "an honest journal marks what it cannot reverse: {entry:?}"
    );
}

#[test]
fn undo_refuses_an_entry_it_cannot_reverse() {
    let f = test_fixtures::linear(3).unwrap();
    let (state, repo) = open(&f);
    state
        .checkout(
            repo,
            &git_engine::CheckoutTarget::Branch {
                name: "main".to_owned(),
            },
        )
        .unwrap();

    assert!(state.undo_last(repo).is_err());
}

#[test]
fn an_undone_entry_is_not_offered_twice() {
    let f = test_fixtures::branched().unwrap();
    let (state, repo) = open(&f);
    state.delete_branch(repo, "dev", true).unwrap();

    state.undo_last(repo).unwrap();

    assert!(
        state.undo_last(repo).is_err(),
        "the same deletion must not be undone twice"
    );
}

#[test]
fn the_newest_entry_is_undone_first() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);
    std::fs::write(f.path().join("a.txt"), "first\n").unwrap();
    state.discard_paths(repo, &["a.txt".to_owned()]).unwrap();
    std::fs::write(f.path().join("b.txt"), "second\n").unwrap();
    state.discard_paths(repo, &["b.txt".to_owned()]).unwrap();

    state.undo_last(repo).unwrap();

    assert!(f.path().join("b.txt").exists());
    assert!(
        !f.path().join("a.txt").exists(),
        "only the newest is undone"
    );
}

#[test]
fn discarding_nothing_records_nothing() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);

    let _ = state.discard_paths(repo, &[]);

    assert!(state.safety_log().is_empty());
}

#[test]
fn a_mutation_does_not_make_the_watcher_report_our_own_writes() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);
    let mut events = state.subscribe();
    std::fs::write(f.path().join("file0.txt"), "edited by us\n").unwrap();
    // Let the edit above settle so only the mutation's own writes are in play.
    std::thread::sleep(std::time::Duration::from_millis(500));
    while events.try_recv().is_ok() {}

    state.stage_paths(repo, &["file0.txt".to_owned()]).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));

    let mut reported = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let app_state::AppEvent::RepoChanged { kind, .. } = event {
            reported.push(kind);
        }
    }
    assert!(
        reported.is_empty(),
        "the UI reloads itself after a mutation; a watcher event on top makes it flicker, got {reported:?}"
    );
}

#[test]
fn an_older_entry_can_be_undone_out_of_order() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);
    f.git(&["branch", "first"]).unwrap();
    f.git(&["branch", "second"]).unwrap();
    let oid = f.oid("first").unwrap();

    state.delete_branch(repo, "first", false).unwrap();
    state.delete_branch(repo, "second", false).unwrap();

    // The recoveries are independent restores, not a stack, so order is the user's choice.
    let older = state
        .safety_log()
        .into_iter()
        .find(|entry| entry.description.contains("first"))
        .unwrap();
    state.undo_entry(repo, older.id).unwrap();

    assert_eq!(f.oid("first").unwrap(), oid);
    assert!(f.oid("second").is_err(), "the newer entry must still stand");
}

#[test]
fn undoing_an_entry_removes_it_from_the_journal() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);
    f.git(&["branch", "gone"]).unwrap();
    state.delete_branch(repo, "gone", false).unwrap();

    let entry = state.safety_log().into_iter().next().unwrap();
    state.undo_entry(repo, entry.id).unwrap();

    assert!(state.safety_log().iter().all(|kept| kept.id != entry.id));
}

#[test]
fn an_unknown_entry_id_is_refused() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);
    assert!(state.undo_entry(repo, 9_999).is_err());
}

#[test]
fn an_entry_belonging_to_another_repository_is_refused() {
    let a = test_fixtures::linear(1).unwrap();
    let b = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let first = state.open_repository(a.path()).unwrap().repo;
    let second = state.open_repository(b.path()).unwrap().repo;

    a.git(&["branch", "doomed"]).unwrap();
    state.delete_branch(first, "doomed", false).unwrap();
    let entry = state.safety_log().into_iter().next().unwrap();

    assert!(state.undo_entry(second, entry.id).is_err());
}

#[test]
fn the_journal_does_not_ship_the_recovery_payload_over_ipc() {
    // A line-level discard keeps the whole patch to undo with. It is the size of the
    // change, it is of no use to the panel, and it crosses the boundary on every read.
    let json = serde_json::to_string(&app_state::SafetyEntry {
        id: 1,
        repo: app_state::RepoId(1),
        description: "Discard lines in a.txt".to_owned(),
        undoable: true,
    })
    .unwrap();

    assert!(!json.contains("recovery"), "{json}");
    assert!(json.contains("undoable"), "{json}");
}

#[test]
fn discarding_a_long_list_of_files_is_undoable() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = open(&f);
    // Short enough for one command line, too long to repeat in the stash message as well.
    let paths: Vec<String> = (0..400)
        .map(|i| format!("a-file-whose-name-is-long-enough-to-matter-{i:04}.txt"))
        .collect();
    for path in &paths {
        std::fs::write(f.path().join(path), "not added yet\n").unwrap();
    }

    state.discard_paths(repo, &paths).unwrap();
    assert!(!f.path().join(&paths[0]).exists());

    state.undo_last(repo).unwrap();

    assert!(paths.iter().all(|path| f.path().join(path).exists()));
}
