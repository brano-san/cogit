// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Investigate: the history of one fragment rather than of one file. Built on `git log -L`,
//! which traces a line range through edits and across renames (R-104).

use git_engine::RepoHandle;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

struct Story {
    fixture: test_fixtures::Fixture,
    wrote: String,
    edited: String,
    unrelated: String,
    edited_again: String,
}

/// Line 2 is written, edited, left alone while another file changes, carried through a
/// rename, and edited once more.
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
    f.commit_staged(13, "rename the file").unwrap();

    f.write_file("tale.txt", "alpha\nBETA2\ngamma\n").unwrap();
    f.git(&["add", "--", "tale.txt"]).unwrap();
    let edited_again = f.commit_staged(14, "edit it once more").unwrap();

    Story {
        fixture: f,
        wrote,
        edited,
        unrelated,
        edited_again,
    }
}

#[test]
fn a_range_reports_the_commits_that_touched_it_newest_first() {
    let story = traced();

    let steps = open(&story.fixture)
        .investigate("tale.txt", 2, 2, 50)
        .unwrap();

    let oids: Vec<&str> = steps.iter().map(|step| step.oid.as_str()).collect();
    assert_eq!(
        oids,
        [&story.edited_again, &story.edited, &story.wrote],
        "{steps:#?}"
    );
}

#[test]
fn a_commit_that_did_not_touch_the_range_is_absent() {
    let story = traced();

    let steps = open(&story.fixture)
        .investigate("tale.txt", 2, 2, 50)
        .unwrap();

    assert!(
        !steps.iter().any(|step| step.oid == story.unrelated),
        "a commit that changed another file has no place here"
    );
}

#[test]
fn each_step_carries_the_diff_of_that_edit() {
    let story = traced();

    let steps = open(&story.fixture)
        .investigate("tale.txt", 2, 2, 50)
        .unwrap();

    assert!(steps[0].diff.contains("-BETA"), "{}", steps[0].diff);
    assert!(steps[0].diff.contains("+BETA2"), "{}", steps[0].diff);
}

#[test]
fn the_range_is_followed_across_a_rename() {
    let story = traced();

    let steps = open(&story.fixture)
        .investigate("tale.txt", 2, 2, 50)
        .unwrap();

    let paths: Vec<&str> = steps.iter().map(|step| step.path.as_str()).collect();
    assert_eq!(
        paths,
        ["tale.txt", "story.txt", "story.txt"],
        "each step carries the path the file had at that commit"
    );
}

#[test]
fn a_step_carries_who_made_it_and_when() {
    let story = traced();

    let steps = open(&story.fixture)
        .investigate("tale.txt", 2, 2, 50)
        .unwrap();

    assert_eq!(steps[0].summary, "edit it once more");
    assert_eq!(steps[0].author, test_fixtures::AUTHOR_NAME);
    assert_eq!(steps[0].email, test_fixtures::AUTHOR_EMAIL);
    assert!(steps[0].timestamp > 0);
}

#[test]
fn the_limit_caps_the_number_of_steps() {
    let story = traced();

    let steps = open(&story.fixture)
        .investigate("tale.txt", 2, 2, 1)
        .unwrap();

    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].oid, story.edited_again);
}

#[test]
fn a_range_that_starts_after_it_ends_is_a_typed_error() {
    let story = traced();

    let failed = open(&story.fixture).investigate("tale.txt", 5, 2, 50);

    assert!(failed.is_err());
}

#[test]
fn a_range_starting_at_zero_is_a_typed_error() {
    let story = traced();

    let failed = open(&story.fixture).investigate("tale.txt", 0, 2, 50);

    assert!(failed.is_err(), "line numbers are 1-based");
}

#[test]
fn a_path_that_does_not_exist_is_a_typed_error() {
    let story = traced();

    let failed = open(&story.fixture).investigate("nowhere.txt", 1, 1, 50);

    assert!(failed.is_err());
}

#[test]
fn a_wider_range_still_reports_every_commit_that_touched_it() {
    let story = traced();

    let steps = open(&story.fixture)
        .investigate("tale.txt", 1, 3, 50)
        .unwrap();

    assert!(
        steps.iter().any(|step| step.oid == story.wrote),
        "{steps:#?}"
    );
}
