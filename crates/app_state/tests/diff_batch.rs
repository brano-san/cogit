// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The batch diff path of doc/08-diff-engine.md section 9, and the cancellation that keeps
//! a diff the user has moved on from out of the way.
//!
//! A file of its own rather than an addition to `diff.rs`: the batch belongs to the
//! `diff-merge` branch and the file beside it does not.

use app_state::{AppState, DiffBatch};
use diff_engine::{DiffOptions, FileDiff, FileDiffEntry};
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

fn ready(batch: DiffBatch) -> Vec<FileDiffEntry> {
    match batch {
        DiffBatch::Ready { files } => files,
        DiffBatch::Superseded => panic!("expected a ready batch, got Superseded"),
    }
}

fn all_three() -> Vec<String> {
    vec![
        "alpha.rs".to_owned(),
        "beta.py".to_owned(),
        "gamma.txt".to_owned(),
    ]
}

#[test]
fn a_commit_touching_three_files_diffs_them_in_one_call() {
    let f = three_changed_files();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    let out = ready(
        state
            .diff_files(
                repo,
                &head_vs_parent(&f),
                &all_three(),
                &DiffOptions::default(),
                1,
            )
            .unwrap(),
    );

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

    let out = ready(
        state
            .diff_files(
                repo,
                &head_vs_parent(&f),
                &paths,
                &DiffOptions::default(),
                1,
            )
            .unwrap(),
    );

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
    let batch = ready(
        state
            .diff_files(repo, &spec, &["beta.py".to_owned()], &options, 1)
            .unwrap(),
    );

    assert_eq!(format!("{:?}", batch[0].diff), format!("{alone:?}"));
}

#[test]
fn the_language_hint_survives_the_batch() {
    let f = three_changed_files();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let paths = vec!["alpha.rs".to_owned(), "beta.py".to_owned()];

    let out = ready(
        state
            .diff_files(
                repo,
                &head_vs_parent(&f),
                &paths,
                &DiffOptions::default(),
                1,
            )
            .unwrap(),
    );

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

    let failed = state.diff_files(
        repo,
        &head_vs_parent(&f),
        &paths,
        &DiffOptions::default(),
        1,
    );

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

    let out = ready(
        state
            .diff_files(repo, &head_vs_parent(&f), &[], &DiffOptions::default(), 1)
            .unwrap(),
    );

    assert!(out.is_empty());
}

#[test]
fn a_request_older_than_one_already_seen_is_superseded() {
    let f = three_changed_files();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let spec = head_vs_parent(&f);
    let options = DiffOptions::default();

    let newer = state
        .diff_files(repo, &spec, &all_three(), &options, 7)
        .unwrap();
    let older = state
        .diff_files(repo, &spec, &all_three(), &options, 3)
        .unwrap();

    assert!(matches!(newer, DiffBatch::Ready { .. }), "{newer:?}");
    assert!(
        matches!(older, DiffBatch::Superseded),
        "the user already moved on: {older:?}"
    );
}

#[test]
fn the_newest_request_keeps_answering() {
    let f = three_changed_files();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let spec = head_vs_parent(&f);
    let options = DiffOptions::default();

    for request in 1..=4 {
        let batch = state
            .diff_files(repo, &spec, &all_three(), &options, request)
            .unwrap();
        assert!(
            matches!(batch, DiffBatch::Ready { .. }),
            "request {request} should still be the newest: {batch:?}"
        );
    }
}

#[test]
fn repeating_the_same_request_number_is_still_current() {
    let f = three_changed_files();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let spec = head_vs_parent(&f);
    let options = DiffOptions::default();

    let first = state
        .diff_files(repo, &spec, &all_three(), &options, 5)
        .unwrap();
    let again = state
        .diff_files(repo, &spec, &all_three(), &options, 5)
        .unwrap();

    assert!(matches!(first, DiffBatch::Ready { .. }), "{first:?}");
    assert!(matches!(again, DiffBatch::Ready { .. }), "{again:?}");
}

#[test]
fn two_repositories_do_not_cancel_each_other() {
    let one = three_changed_files();
    let two = three_changed_files();
    let state = AppState::new();
    let repo_one = state.open_repository(one.path()).unwrap().repo;
    let repo_two = state.open_repository(two.path()).unwrap().repo;
    let options = DiffOptions::default();

    let high = state
        .diff_files(repo_one, &head_vs_parent(&one), &all_three(), &options, 99)
        .unwrap();
    let low = state
        .diff_files(repo_two, &head_vs_parent(&two), &all_three(), &options, 1)
        .unwrap();

    assert!(matches!(high, DiffBatch::Ready { .. }), "{high:?}");
    assert!(
        matches!(low, DiffBatch::Ready { .. }),
        "a request for another repository must not be cancelled: {low:?}"
    );
}

#[test]
fn two_app_states_do_not_cancel_each_other() {
    let f = three_changed_files();
    let first = AppState::new();
    let second = AppState::new();
    let repo_first = first.open_repository(f.path()).unwrap().repo;
    let repo_second = second.open_repository(f.path()).unwrap().repo;
    let spec = head_vs_parent(&f);
    let options = DiffOptions::default();

    let high = first
        .diff_files(repo_first, &spec, &all_three(), &options, 50)
        .unwrap();
    let low = second
        .diff_files(repo_second, &spec, &all_three(), &options, 2)
        .unwrap();

    assert!(matches!(high, DiffBatch::Ready { .. }), "{high:?}");
    assert!(
        matches!(low, DiffBatch::Ready { .. }),
        "another AppState must keep its own request numbering: {low:?}"
    );
}
