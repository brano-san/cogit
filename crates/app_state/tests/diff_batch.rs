// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The batch diff path of doc/08-diff-engine.md section 9.
//!
//! A file of its own rather than an addition to `diff.rs`: the batch belongs to the
//! `diff-merge` branch and the file beside it does not.

use app_state::AppState;
use diff_engine::{DiffOptions, FileDiff};
use git_engine::DiffSpec;

/// A commit that rewrites three files at once, which is what the batch exists for.
fn three_changed_files() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(1).unwrap();
    for (name, body) in [
        ("alpha.rs", "fn alpha() {}\nfn extra() {}\n"),
        (
            "beta.py",
            "def beta():\n    pass\n\ndef extra():\n    pass\n",
        ),
        ("gamma.txt", "gamma\nextra\n"),
    ] {
        std::fs::write(f.path().join(name), body).unwrap();
        f.git(&["add", "--", name]).unwrap();
    }
    f.commit_staged(1, "touch three files").unwrap();
    f
}

fn head_vs_parent(f: &test_fixtures::Fixture) -> DiffSpec {
    DiffSpec::CommitVsParent {
        oid: f.oid("HEAD").unwrap(),
    }
}

fn hunks(diff: &FileDiff) -> usize {
    match diff {
        FileDiff::Text { hunks, .. } => hunks.len(),
        other => panic!("expected a text diff, got {other:?}"),
    }
}

#[test]
fn a_commit_touching_three_files_diffs_them_in_one_call() {
    let f = three_changed_files();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let paths = vec![
        "alpha.rs".to_owned(),
        "beta.py".to_owned(),
        "gamma.txt".to_owned(),
    ];

    let out = state
        .diff_files(repo, &head_vs_parent(&f), &paths, &DiffOptions::default())
        .unwrap();

    assert_eq!(out.len(), 3);
    for entry in &out {
        assert_eq!(hunks(&entry.diff), 1);
    }
}

#[test]
fn the_batch_answers_in_the_order_it_was_asked() {
    let f = three_changed_files();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let paths = vec![
        "gamma.txt".to_owned(),
        "alpha.rs".to_owned(),
        "beta.py".to_owned(),
    ];

    let out = state
        .diff_files(repo, &head_vs_parent(&f), &paths, &DiffOptions::default())
        .unwrap();

    let answered: Vec<&str> = out.iter().map(|entry| entry.path.as_str()).collect();
    assert_eq!(answered, ["gamma.txt", "alpha.rs", "beta.py"]);
}

#[test]
fn the_batch_agrees_with_one_file_at_a_time() {
    let f = three_changed_files();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let spec = head_vs_parent(&f);
    let options = DiffOptions::default();

    let alone = state.diff_file(repo, &spec, "beta.py", &options).unwrap();
    let batch = state
        .diff_files(repo, &spec, &["beta.py".to_owned()], &options)
        .unwrap();

    assert_eq!(format!("{:?}", batch[0].diff), format!("{alone:?}"));
}

#[test]
fn the_language_hint_survives_the_batch() {
    let f = three_changed_files();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let paths = vec!["alpha.rs".to_owned(), "beta.py".to_owned()];

    let out = state
        .diff_files(repo, &head_vs_parent(&f), &paths, &DiffOptions::default())
        .unwrap();

    let languages: Vec<Option<&str>> = out
        .iter()
        .map(|entry| match &entry.diff {
            FileDiff::Text { language, .. } => language.as_deref(),
            _ => None,
        })
        .collect();
    assert_eq!(languages, [Some("rust"), Some("python")]);
}

#[test]
fn a_path_absent_from_both_sides_is_a_typed_error() {
    let f = three_changed_files();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let paths = vec!["alpha.rs".to_owned(), "nowhere.rs".to_owned()];

    let failed = state.diff_files(repo, &head_vs_parent(&f), &paths, &DiffOptions::default());

    assert!(
        failed.is_err(),
        "a garbage path must not be dropped silently"
    );
}

#[test]
fn an_empty_request_is_an_empty_answer_rather_than_an_error() {
    let f = three_changed_files();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let out = state
        .diff_files(repo, &head_vs_parent(&f), &[], &DiffOptions::default())
        .unwrap();

    assert!(out.is_empty());
}
