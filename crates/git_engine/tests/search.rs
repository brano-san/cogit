// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{CommitQuery, CommitRow, RepoHandle};
use test_fixtures::BASE_TIMESTAMP;

const STEP: i64 = 60;

fn open(fixture: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(fixture.path()).unwrap()
}

fn search(repo: &RepoHandle, query: &CommitQuery) -> Vec<CommitRow> {
    let mut found = Vec::new();
    repo.search_commits(query, 100, |chunk| {
        found.extend(chunk);
        true
    })
    .unwrap();
    found
}

fn summaries(rows: &[CommitRow]) -> Vec<&str> {
    rows.iter().map(|r| r.summary.as_str()).collect()
}

#[test]
fn an_empty_query_returns_the_whole_history() {
    let f = test_fixtures::linear(5).unwrap();
    let repo = open(&f);

    assert_eq!(search(&repo, &CommitQuery::default()).len(), 5);
}

#[test]
fn filters_by_message_substring() {
    let f = test_fixtures::linear(5).unwrap();
    let repo = open(&f);

    let query = CommitQuery {
        message: Some("commit 3".to_owned()),
        ..CommitQuery::default()
    };

    assert_eq!(summaries(&search(&repo, &query)), ["commit 3"]);
}

#[test]
fn the_message_filter_ignores_case() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);

    let query = CommitQuery {
        message: Some("COMMIT".to_owned()),
        ..CommitQuery::default()
    };

    assert_eq!(search(&repo, &query).len(), 2);
}

#[test]
fn filters_by_author() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);

    let mine = CommitQuery {
        author: Some("fixture".to_owned()),
        ..CommitQuery::default()
    };
    let theirs = CommitQuery {
        author: Some("nobody".to_owned()),
        ..CommitQuery::default()
    };

    assert_eq!(search(&repo, &mine).len(), 3);
    assert!(search(&repo, &theirs).is_empty());
}

#[test]
fn the_author_filter_also_matches_the_email() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    let query = CommitQuery {
        author: Some("@cogit.test".to_owned()),
        ..CommitQuery::default()
    };

    assert_eq!(search(&repo, &query).len(), 1);
}

#[test]
fn filters_by_oid_prefix() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);
    let head = f.oid("HEAD").unwrap();

    let query = CommitQuery {
        oid_prefix: Some(head[..7].to_owned()),
        ..CommitQuery::default()
    };

    let found = search(&repo, &query);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].oid, head);
}

#[test]
fn filters_by_date_range_inclusively() {
    let f = test_fixtures::linear(5).unwrap();
    let repo = open(&f);

    let query = CommitQuery {
        since: Some(BASE_TIMESTAMP + STEP),
        until: Some(BASE_TIMESTAMP + 3 * STEP),
        ..CommitQuery::default()
    };

    assert_eq!(
        summaries(&search(&repo, &query)),
        ["commit 3", "commit 2", "commit 1"]
    );
}

#[test]
fn filters_by_the_path_a_commit_touched() {
    let f = test_fixtures::linear(4).unwrap();
    let repo = open(&f);

    let query = CommitQuery {
        path: Some("file2.txt".to_owned()),
        ..CommitQuery::default()
    };

    assert_eq!(summaries(&search(&repo, &query)), ["commit 2"]);
}

#[test]
fn the_path_filter_follows_a_file_through_later_edits() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::write(f.path().join("file0.txt"), "edited\n").unwrap();
    f.git(&["add", "--", "file0.txt"]).unwrap();
    f.commit_staged(2, "edit file0 again").unwrap();
    let repo = open(&f);

    let query = CommitQuery {
        path: Some("file0.txt".to_owned()),
        ..CommitQuery::default()
    };

    assert_eq!(
        summaries(&search(&repo, &query)),
        ["edit file0 again", "commit 0"]
    );
}

#[test]
fn a_path_nobody_touched_matches_nothing() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);

    let query = CommitQuery {
        path: Some("never-existed.txt".to_owned()),
        ..CommitQuery::default()
    };

    assert!(search(&repo, &query).is_empty());
}

#[test]
fn conditions_combine_with_and() {
    let f = test_fixtures::linear(5).unwrap();
    let repo = open(&f);

    let matching = CommitQuery {
        message: Some("commit 2".to_owned()),
        path: Some("file2.txt".to_owned()),
        ..CommitQuery::default()
    };
    let contradictory = CommitQuery {
        message: Some("commit 2".to_owned()),
        path: Some("file3.txt".to_owned()),
        ..CommitQuery::default()
    };

    assert_eq!(search(&repo, &matching).len(), 1);
    assert!(
        search(&repo, &contradictory).is_empty(),
        "conditions are ANDed, not ORed"
    );
}

#[test]
fn results_arrive_in_chunks_of_the_requested_size() {
    let f = test_fixtures::linear(7).unwrap();
    let repo = open(&f);

    let mut sizes = Vec::new();
    repo.search_commits(&CommitQuery::default(), 3, |chunk| {
        sizes.push(chunk.len());
        true
    })
    .unwrap();

    assert_eq!(sizes, [3, 3, 1]);
}

#[test]
fn returning_false_stops_the_walk() {
    let f = test_fixtures::linear(20).unwrap();
    let repo = open(&f);

    let mut seen = 0;
    repo.search_commits(&CommitQuery::default(), 2, |chunk| {
        seen += chunk.len();
        false
    })
    .unwrap();

    assert_eq!(
        seen, 2,
        "cancelling after the first chunk must stop the walk"
    );
}

#[test]
fn an_empty_repository_yields_nothing_without_panicking() {
    let f = test_fixtures::empty().unwrap();
    let repo = open(&f);

    assert!(search(&repo, &CommitQuery::default()).is_empty());
}

#[test]
fn a_merge_is_matched_against_its_first_parent_for_paths() {
    let f = test_fixtures::diamond().unwrap();
    let repo = open(&f);

    let query = CommitQuery {
        path: Some("dev.txt".to_owned()),
        ..CommitQuery::default()
    };

    let rows = search(&repo, &query);
    let found = summaries(&rows);
    assert!(
        found.contains(&"merge dev into main"),
        "the merge brought dev.txt onto main, got {found:?}"
    );
}
