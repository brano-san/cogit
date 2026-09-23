// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::RepoHandle;
use test_fixtures::{BASE_TIMESTAMP, Fixture};

const STEP: i64 = 60;

fn at(index: i64) -> i64 {
    BASE_TIMESTAMP + index * STEP
}

fn date_of(repo: &RepoHandle, full_name: &str) -> Option<i64> {
    repo.ref_dates()
        .unwrap()
        .into_iter()
        .find(|entry| entry.full_name == full_name)
        .map(|entry| entry.timestamp)
}

/// Commits 0..3 at their fixture times; `old` on the first, `v2` annotated much later.
fn dated() -> Fixture {
    let f = test_fixtures::linear(3).unwrap();
    f.git(&["branch", "topic", "HEAD~2"]).unwrap();
    f.git(&["tag", "old", "HEAD~1"]).unwrap();
    f.git_at(40, &["tag", "-a", "v2", "-m", "release two"])
        .unwrap();
    f
}

#[test]
fn the_tag_separator_is_a_slash_when_the_repository_does_not_set_one() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    assert_eq!(repo.tag_group_separator(), "/");
}

#[test]
fn the_tag_separator_comes_from_the_repository_config() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["config", "cogit.tagGroupSeparator", "-"]).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    assert_eq!(repo.tag_group_separator(), "-");
}

#[test]
fn an_empty_tag_separator_is_kept_and_means_no_folders() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["config", "cogit.tagGroupSeparator", ""]).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    assert_eq!(repo.tag_group_separator(), "");
}

#[test]
fn a_branch_is_dated_by_its_tip_commit() {
    let f = dated();
    let repo = RepoHandle::open(f.path()).unwrap();
    let main = date_of(&repo, "refs/heads/main").unwrap();
    let topic = date_of(&repo, "refs/heads/topic").unwrap();
    assert!(topic < main, "topic sits two commits behind main");
}

#[test]
fn a_lightweight_tag_is_dated_by_its_commit() {
    let f = dated();
    let repo = RepoHandle::open(f.path()).unwrap();
    let old = date_of(&repo, "refs/tags/old").unwrap();
    let main = date_of(&repo, "refs/heads/main").unwrap();
    let topic = date_of(&repo, "refs/heads/topic").unwrap();
    assert!(topic < old && old < main);
}

#[test]
fn an_annotated_tag_is_dated_by_the_tag_not_the_commit() {
    let f = dated();
    let repo = RepoHandle::open(f.path()).unwrap();
    assert_eq!(date_of(&repo, "refs/tags/v2"), Some(at(40)));
}

#[test]
fn remote_branches_are_dated_too() {
    let f = test_fixtures::with_remote().unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    assert_eq!(date_of(&repo, "refs/remotes/origin/main"), Some(at(30)));
}
