#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The References panel drives the graph: only the refs it has ticked are walked (T5.9).

use git_engine::{CommitQuery, RepoHandle};

fn summaries(repo: &RepoHandle, query: &CommitQuery) -> Vec<String> {
    let mut seen = Vec::new();
    repo.search_commits(query, 100, |chunk| {
        seen.extend(chunk.into_iter().map(|row| row.summary));
        true
    })
    .unwrap();
    seen
}

fn tips(names: &[&str]) -> CommitQuery {
    CommitQuery {
        tips: names.iter().map(|name| (*name).to_owned()).collect(),
        ..CommitQuery::default()
    }
}

#[test]
fn an_empty_tip_list_still_walks_every_ref() {
    let f = test_fixtures::branched().unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    let all = summaries(&repo, &CommitQuery::default());
    assert!(all.len() > 1, "{all:?}");
}

#[test]
fn one_tip_walks_only_that_branch() {
    let f = test_fixtures::diamond().unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    let everything = summaries(&repo, &CommitQuery::default());
    let dev = summaries(&repo, &tips(&["dev"]));

    assert_eq!(dev, ["commit 1", "commit 0"], "{everything:?}");
    assert!(dev.len() < everything.len(), "{dev:?} vs {everything:?}");
}

#[test]
fn two_tips_join_their_histories_without_repeating_the_shared_part() {
    let f = test_fixtures::diamond().unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    let both = summaries(&repo, &tips(&["dev", "main"]));
    let mut unique = both.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(both.len(), 4, "{both:?}");
    assert_eq!(
        both.len(),
        unique.len(),
        "the shared base must appear once: {both:?}"
    );
    assert!(both.contains(&"merge dev into main".to_owned()), "{both:?}");
}

#[test]
fn a_raw_oid_is_a_valid_tip_so_a_lost_commit_can_be_shown() {
    let f = test_fixtures::linear(3).unwrap();
    let oid = f.oid("HEAD~1").unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    let seen = summaries(&repo, &tips(&[&oid]));
    assert_eq!(seen, ["commit 1", "commit 0"]);
}

#[test]
fn a_tip_that_cannot_be_resolved_is_skipped_rather_than_fatal() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    let seen = summaries(&repo, &tips(&["no-such-branch", "HEAD"]));
    assert_eq!(seen, ["commit 1", "commit 0"]);
}

#[test]
fn every_tip_failing_leaves_an_empty_graph_rather_than_the_whole_history() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    assert!(summaries(&repo, &tips(&["nope"])).is_empty());
}

#[test]
fn tips_combine_with_the_other_filters() {
    let f = test_fixtures::linear(4).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    let query = CommitQuery {
        message: Some("commit 1".to_owned()),
        ..tips(&["HEAD"])
    };
    assert_eq!(summaries(&repo, &query), ["commit 1"]);
}
