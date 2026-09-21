#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Surgery on a commit the team already shares is refused, not warned about: a force-push
//! over `main` costs everyone who has it a divergence (M12).

use app_state::AppState;

fn opened(f: &test_fixtures::Fixture) -> (AppState, app_state::RepoId) {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    (state, repo)
}

const PUSHED: &str = "HEAD~2";

#[test]
fn the_refs_protecting_a_commit_are_reported() {
    let f = test_fixtures::with_remote().unwrap();
    let (state, repo) = opened(&f);
    assert!(!state.protecting_refs(repo, PUSHED).unwrap().is_empty());
}

#[test]
fn an_unpushed_commit_is_protected_by_nothing() {
    let f = test_fixtures::linear(2).unwrap();
    let (state, repo) = opened(&f);
    assert!(state.protecting_refs(repo, "HEAD").unwrap().is_empty());
}

#[test]
fn splitting_a_protected_commit_is_refused() {
    let f = test_fixtures::with_remote().unwrap();
    let (state, repo) = opened(&f);

    let refused = state.split_off(repo, PUSHED, &["file0.txt".to_owned()], "split", true);

    assert!(refused.is_err(), "the split went through");
}

#[test]
fn the_refusal_names_the_branch_so_the_user_knows_why() {
    let f = test_fixtures::with_remote().unwrap();
    let (state, repo) = opened(&f);

    let message = state
        .split_off(repo, PUSHED, &["file0.txt".to_owned()], "split", true)
        .unwrap_err()
        .to_string();

    assert!(message.contains("origin/main"), "{message}");
}

#[test]
fn splitting_an_unpushed_commit_is_still_allowed() {
    let f = test_fixtures::with_remote().unwrap();
    for name in ["one.txt", "two.txt"] {
        std::fs::write(
            f.path().join(name),
            "body
",
        )
        .unwrap();
        f.git(&["add", "--", name]).unwrap();
    }
    f.git(&["commit", "-m", "two files, never pushed"]).unwrap();
    let (state, repo) = opened(&f);

    let outcome = state.split_off(repo, "HEAD", &["one.txt".to_owned()], "split", true);
    assert!(outcome.is_ok(), "{outcome:?}");
}

#[test]
fn rolling_back_is_not_refused_because_it_rewrites_nothing() {
    let f = test_fixtures::with_remote().unwrap();
    let (state, repo) = opened(&f);
    assert!(state.rollback_to(repo, PUSHED, &[]).is_ok());
}
