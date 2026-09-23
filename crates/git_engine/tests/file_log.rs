// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The Investigate window's Navigation log: every commit that changed one file, followed
//! across renames the way `git log --follow` does.

use git_engine::{FileChange, RepoHandle};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

struct Story {
    fixture: test_fixtures::Fixture,
    wrote: String,
    edited: String,
    unrelated: String,
    renamed: String,
    edited_again: String,
}

fn story() -> Story {
    let f = test_fixtures::linear(1).unwrap();

    f.write_file("story.txt", "alpha\nbeta\ngamma\n").unwrap();
    f.git(&["add", "--", "story.txt"]).unwrap();
    let wrote = f.commit_staged(10, "write the story").unwrap();

    f.write_file("story.txt", "alpha\nBETA\ngamma\n").unwrap();
    f.git(&["add", "--", "story.txt"]).unwrap();
    let edited = f.commit_staged(11, "edit the story").unwrap();

    f.write_file("elsewhere.txt", "nothing to do with it\n")
        .unwrap();
    f.git(&["add", "--", "elsewhere.txt"]).unwrap();
    let unrelated = f.commit_staged(12, "touch another file").unwrap();

    f.git(&["mv", "story.txt", "tale.txt"]).unwrap();
    let renamed = f.commit_staged(13, "rename the story").unwrap();

    f.write_file("tale.txt", "alpha\nBETA\ngamma\ndelta\n")
        .unwrap();
    f.git(&["add", "--", "tale.txt"]).unwrap();
    let edited_again = f.commit_staged(14, "extend the tale").unwrap();

    Story {
        fixture: f,
        wrote,
        edited,
        unrelated,
        renamed,
        edited_again,
    }
}

#[test]
fn lists_the_commits_that_changed_the_file_newest_first_across_a_rename() {
    let s = story();

    let log = open(&s.fixture)
        .file_log("tale.txt", None, true, 100)
        .unwrap();

    let oids: Vec<&str> = log.iter().map(|rev| rev.oid.as_str()).collect();
    assert_eq!(oids, [&s.edited_again, &s.renamed, &s.edited, &s.wrote]);
    assert!(!oids.contains(&s.unrelated.as_str()));
}

#[test]
fn each_entry_names_the_file_as_it_was_called_at_that_commit() {
    let s = story();

    let log = open(&s.fixture)
        .file_log("tale.txt", None, true, 100)
        .unwrap();

    let paths: Vec<&str> = log.iter().map(|rev| rev.path.as_str()).collect();
    assert_eq!(paths, ["tale.txt", "tale.txt", "story.txt", "story.txt"]);
}

#[test]
fn the_rename_carries_the_old_name_and_the_first_commit_is_an_addition() {
    let s = story();

    let log = open(&s.fixture)
        .file_log("tale.txt", None, true, 100)
        .unwrap();

    assert_eq!(log[1].change, FileChange::Renamed);
    assert_eq!(log[1].previous_path.as_deref(), Some("story.txt"));
    assert_eq!(log[0].change, FileChange::Modified);
    assert_eq!(log[3].change, FileChange::Added);
    assert_eq!(log[3].previous_path, None);
}

#[test]
fn without_follow_the_log_stops_at_the_rename() {
    let s = story();

    let log = open(&s.fixture)
        .file_log("tale.txt", None, false, 100)
        .unwrap();

    let oids: Vec<&str> = log.iter().map(|rev| rev.oid.as_str()).collect();
    assert_eq!(oids, [&s.edited_again, &s.renamed]);
}

#[test]
fn starting_from_an_older_commit_leaves_the_newer_ones_out() {
    let s = story();

    let log = open(&s.fixture)
        .file_log("story.txt", Some(&s.edited), true, 100)
        .unwrap();

    let oids: Vec<&str> = log.iter().map(|rev| rev.oid.as_str()).collect();
    assert_eq!(oids, [&s.edited, &s.wrote]);
}

#[test]
fn entries_carry_author_date_subject_and_parents() {
    let s = story();

    let log = open(&s.fixture)
        .file_log("tale.txt", None, true, 100)
        .unwrap();

    assert_eq!(log[0].summary, "extend the tale");
    assert_eq!(log[0].author, test_fixtures::AUTHOR_NAME);
    assert_eq!(log[0].email, test_fixtures::AUTHOR_EMAIL);
    assert!(log[0].timestamp > 0);
    assert_eq!(log[0].parents, std::slice::from_ref(&s.renamed));
}

#[test]
fn the_limit_caps_the_log() {
    let s = story();

    let log = open(&s.fixture)
        .file_log("tale.txt", None, true, 2)
        .unwrap();

    assert_eq!(log.len(), 2);
}

#[test]
fn a_path_that_never_existed_has_an_empty_log() {
    let s = story();

    let log = open(&s.fixture)
        .file_log("nowhere.txt", None, true, 100)
        .unwrap();

    assert!(log.is_empty());
}

#[test]
fn an_unborn_branch_has_an_empty_log_rather_than_an_error() {
    let f = test_fixtures::empty().unwrap();

    let log = open(&f).file_log("anything.txt", None, true, 100).unwrap();

    assert!(log.is_empty());
}

#[test]
fn a_name_with_spaces_and_unicode_survives_the_round_trip() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("dir/naïve name.txt", "one\n").unwrap();
    f.git(&["add", "--", "dir/naïve name.txt"]).unwrap();
    let added = f.commit_staged(10, "add").unwrap();

    let log = open(&f)
        .file_log("dir/naïve name.txt", None, true, 100)
        .unwrap();

    assert_eq!(log.len(), 1);
    assert_eq!(log[0].oid, added);
    assert_eq!(log[0].path, "dir/naïve name.txt");
}
