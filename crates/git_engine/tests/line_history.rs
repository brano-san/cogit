// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! What the Blame window needs beyond blame itself: the versions of the file it can show,
//! and the history of the line under the cursor (R-202).

use git_engine::RepoHandle;

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
    pushed_down: String,
}

/// Line 2 is written, edited, left alone while another file changes, carried through a
/// rename, edited once more, and finally pushed down to line 3 by a line above it.
fn traced() -> Story {
    let f = test_fixtures::linear(1).unwrap();

    f.write_file("story.txt", "alpha\nbeta\ngamma\n").unwrap();
    f.git(&["add", "--", "story.txt"]).unwrap();
    let wrote = f.commit_staged(10, "write the fragment").unwrap();

    f.write_file("story.txt", "alpha\nBETA\ngamma\n").unwrap();
    f.git(&["add", "--", "story.txt"]).unwrap();
    let edited = f.commit_staged(11, "edit the fragment").unwrap();

    f.write_file("elsewhere.txt", "nothing to do with it\n")
        .unwrap();
    f.git(&["add", "--", "elsewhere.txt"]).unwrap();
    let unrelated = f.commit_staged(12, "touch another file").unwrap();

    f.git(&["mv", "story.txt", "tale.txt"]).unwrap();
    let renamed = f.commit_staged(13, "rename the file").unwrap();

    f.write_file("tale.txt", "alpha\nBETA2\ngamma\n").unwrap();
    f.git(&["add", "--", "tale.txt"]).unwrap();
    let edited_again = f.commit_staged(14, "edit it once more").unwrap();

    f.write_file("tale.txt", "zero\nalpha\nBETA2\ngamma\n")
        .unwrap();
    f.git(&["add", "--", "tale.txt"]).unwrap();
    let pushed_down = f.commit_staged(15, "add a line above").unwrap();

    Story {
        fixture: f,
        wrote,
        edited,
        unrelated,
        renamed,
        edited_again,
        pushed_down,
    }
}

#[test]
fn a_line_lists_the_commits_that_changed_it_newest_first() {
    let story = traced();

    let history = open(&story.fixture)
        .line_history("tale.txt", "HEAD", 3, 50)
        .unwrap();

    let oids: Vec<&str> = history.iter().map(|version| version.oid.as_str()).collect();
    assert_eq!(
        oids,
        [&story.edited_again, &story.edited, &story.wrote],
        "a line added above moves it but does not change it: {history:#?}"
    );
}

#[test]
fn each_version_carries_the_line_as_that_commit_left_it() {
    let story = traced();

    let history = open(&story.fixture)
        .line_history("tale.txt", "HEAD", 3, 50)
        .unwrap();

    let texts: Vec<&str> = history
        .iter()
        .map(|version| version.text.as_str())
        .collect();
    assert_eq!(texts, ["BETA2", "BETA", "beta"]);
}

#[test]
fn each_version_says_where_the_line_stood_then() {
    let story = traced();

    let history = open(&story.fixture)
        .line_history("tale.txt", "HEAD", 3, 50)
        .unwrap();

    let lines: Vec<u32> = history.iter().map(|version| version.line).collect();
    assert_eq!(
        lines,
        [2, 2, 2],
        "line 3 today was line 2 before the insertion"
    );
}

#[test]
fn the_line_is_followed_across_a_rename() {
    let story = traced();

    let history = open(&story.fixture)
        .line_history("tale.txt", "HEAD", 3, 50)
        .unwrap();

    let paths: Vec<&str> = history
        .iter()
        .map(|version| version.path.as_str())
        .collect();
    assert_eq!(paths, ["tale.txt", "story.txt", "story.txt"]);
}

#[test]
fn the_history_starts_at_the_version_being_viewed() {
    let story = traced();

    let history = open(&story.fixture)
        .line_history("story.txt", &story.edited, 2, 50)
        .unwrap();

    let oids: Vec<&str> = history.iter().map(|version| version.oid.as_str()).collect();
    assert_eq!(oids, [&story.edited, &story.wrote]);
}

#[test]
fn a_version_carries_who_made_it_and_when() {
    let story = traced();

    let history = open(&story.fixture)
        .line_history("tale.txt", "HEAD", 3, 50)
        .unwrap();

    assert_eq!(history[0].summary, "edit it once more");
    assert_eq!(history[0].author, test_fixtures::AUTHOR_NAME);
    assert_eq!(history[0].email, test_fixtures::AUTHOR_EMAIL);
    assert!(history[0].timestamp > 0);
}

#[test]
fn the_limit_caps_the_history() {
    let story = traced();

    let history = open(&story.fixture)
        .line_history("tale.txt", "HEAD", 3, 1)
        .unwrap();

    assert_eq!(history.len(), 1);
}

#[test]
fn line_zero_is_a_typed_error() {
    let story = traced();

    assert!(
        open(&story.fixture)
            .line_history("tale.txt", "HEAD", 0, 50)
            .is_err()
    );
}

#[test]
fn a_line_past_the_end_is_a_typed_error() {
    let story = traced();

    assert!(
        open(&story.fixture)
            .line_history("tale.txt", "HEAD", 99, 50)
            .is_err()
    );
}

#[test]
fn the_versions_of_a_file_are_the_commits_that_changed_it_newest_first() {
    let story = traced();

    let revisions = open(&story.fixture)
        .file_revisions("tale.txt", "HEAD", 100)
        .unwrap();

    let oids: Vec<&str> = revisions.iter().map(|row| row.oid.as_str()).collect();
    assert_eq!(
        oids,
        [&story.pushed_down, &story.edited_again, &story.renamed],
        "{revisions:#?}"
    );
}

#[test]
fn the_versions_end_at_the_revision_asked_for() {
    let story = traced();

    let revisions = open(&story.fixture)
        .file_revisions("story.txt", &story.unrelated, 100)
        .unwrap();

    let oids: Vec<&str> = revisions.iter().map(|row| row.oid.as_str()).collect();
    assert_eq!(oids, [&story.edited, &story.wrote]);
}

#[test]
fn a_commit_that_left_the_file_alone_is_not_a_version_of_it() {
    let story = traced();

    let revisions = open(&story.fixture)
        .file_revisions("story.txt", &story.unrelated, 100)
        .unwrap();

    assert!(!revisions.iter().any(|row| row.oid == story.unrelated));
}

#[test]
fn the_limit_caps_the_versions() {
    let story = traced();

    let revisions = open(&story.fixture)
        .file_revisions("tale.txt", "HEAD", 2)
        .unwrap();

    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].oid, story.pushed_down);
}
