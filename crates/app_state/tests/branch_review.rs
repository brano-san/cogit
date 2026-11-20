#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, LineCounts, ReviewPath};
use git_engine::DiffSpec;

fn counted(added: u32, removed: u32, path: &str) -> LineCounts {
    LineCounts {
        path: path.to_owned(),
        added: Some(added),
        removed: Some(removed),
    }
}

fn file(path: &str, old_path: Option<&str>) -> ReviewPath {
    ReviewPath {
        path: path.to_owned(),
        old_path: old_path.map(str::to_owned),
    }
}

#[test]
fn line_counts_follow_renames_and_skip_binary_files() {
    let f = test_fixtures::Fixture::init().unwrap();
    let body: String = (0..30).map(|i| format!("line {i}\n")).collect();
    f.commit_file(0, "old.txt", &body).unwrap();
    f.commit_file(1, "gone.txt", "a\nb\n").unwrap();
    f.git(&["switch", "-c", "topic"]).unwrap();
    f.git(&["mv", "old.txt", "new.txt"]).unwrap();
    f.write_file("new.txt", &format!("{body}line 30\n"))
        .unwrap();
    f.git(&["rm", "-q", "gone.txt"]).unwrap();
    f.write_file("blob.bin", "\0\u{1}\u{2}").unwrap();
    f.git(&["add", "."]).unwrap();
    f.commit_staged(2, "work").unwrap();

    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let review = state.branch_review(repo, "main", "topic").unwrap();
    let spec = DiffSpec::CommitVsCommit {
        a: review.merge_base.clone(),
        b: review.branch_oid.clone(),
    };
    let counts = state
        .review_line_counts(
            repo,
            &spec,
            &[
                file("new.txt", Some("old.txt")),
                file("gone.txt", None),
                file("blob.bin", None),
            ],
        )
        .unwrap();
    assert_eq!(
        counts,
        [
            counted(1, 0, "new.txt"),
            counted(0, 2, "gone.txt"),
            LineCounts {
                path: "blob.bin".to_owned(),
                added: None,
                removed: None
            }
        ]
    );
}

#[test]
fn the_default_base_and_the_files_go_through_the_state() {
    let f = test_fixtures::branched().unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let base = state.review_base(repo, "dev").unwrap().unwrap();
    let review = state.branch_review(repo, &base, "dev").unwrap();
    let files = state
        .review_files(repo, &review.merge_base, "dev", false)
        .unwrap();
    assert_eq!(base, "refs/heads/main");
    assert_eq!(files.len(), 2);
}

#[test]
fn commit_filter_lists_only_the_files_changed_since_the_first_parent() {
    let f = test_fixtures::branched().unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let review = state.branch_review(repo, "main", "dev").unwrap();

    assert_eq!(review.commits.len(), 2);
    let paths: Vec<_> = review
        .commits
        .iter()
        .map(|commit| {
            state
                .commit_files(repo, &commit.oid)
                .unwrap()
                .into_iter()
                .map(|file| file.path)
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(paths, [["dev-2.txt".to_owned()], ["dev-1.txt".to_owned()]]);
}
