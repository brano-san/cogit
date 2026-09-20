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
