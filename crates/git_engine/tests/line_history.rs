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

// The path each version is shown under comes from the `+++ b/…` line of `log -L`, whose
// form could depend on the user's config. Pinned with the settings that would change it:
// quoting on (git's default, as in every fixture), `diff.noprefix`, and a folder that
// looks like the prefix.
#[test]
fn the_path_of_each_version_is_the_file_itself_whatever_its_name() {
    let f = test_fixtures::unicode_paths().unwrap();
    f.write_file("b/inner.txt", "in a folder called b\n")
        .unwrap();
    f.git(&["add", "--", "b/inner.txt"]).unwrap();
    f.commit_staged(10, "a folder that looks like a prefix")
        .unwrap();
    f.git(&["config", "diff.noprefix", "true"]).unwrap();
    let repo = open(&f);

    for path in [
        "файл.txt",
        "with space.txt",
        "каталог/вложенный.txt",
        "b/inner.txt",
    ] {
        let versions = repo.line_history(path, "HEAD", 1, 10).unwrap();
        assert!(!versions.is_empty(), "{path}: no history");
        assert!(
            versions.iter().all(|v| v.path == path),
            "{path}: {versions:?}"
        );
    }
}

// The graph's own order (R-766) reached the versions too: a side branch older than the main
// line came before it, and the list was no longer newest first.
#[test]
fn the_versions_are_newest_first_across_a_merge() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["switch", "-c", "side"]).unwrap();
    f.write_file("dir/side.txt", "side\n").unwrap();
    f.git(&["add", "--", "dir/side.txt"]).unwrap();
    let side = f.commit_staged(10, "on the side").unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.write_file("dir/main.txt", "main\n").unwrap();
    f.git(&["add", "--", "dir/main.txt"]).unwrap();
    let main = f.commit_staged(20, "on main").unwrap();
    f.git(&["merge", "--no-ff", "--no-commit", "side"]).unwrap();
    let merge = f.commit_staged(30, "merge the side").unwrap();

    let revisions = open(&f).file_revisions("dir", "HEAD", 100).unwrap();

    let oids: Vec<&str> = revisions.iter().map(|row| row.oid.as_str()).collect();
    assert_eq!(oids, [&merge, &main, &side], "{revisions:#?}");
}

// `git log -- run.sh` lists a commit that only makes it executable; the filter compared
// the blob alone and left it out.
#[test]
fn a_commit_that_only_changes_the_mode_is_a_version() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("run.sh", "echo hi\n").unwrap();
    f.git(&["add", "--", "run.sh"]).unwrap();
    let written = f.commit_staged(10, "write the script").unwrap();
    f.git(&["update-index", "--chmod=+x", "--", "run.sh"])
        .unwrap();
    let executable = f.commit_staged(11, "make it executable").unwrap();

    let revisions = open(&f).file_revisions("run.sh", "HEAD", 100).unwrap();

    let oids: Vec<&str> = revisions.iter().map(|row| row.oid.as_str()).collect();
    assert_eq!(oids, [&executable, &written]);
}

// `git log -L` prints `--- a/a.txt` before `+++ b/b.txt` for a commit that moves and edits
// the file: the version was recorded at the old path, where that commit has no such file.
#[test]
fn a_version_that_moved_and_edited_the_file_is_at_its_new_path() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("a.txt", "one\ntwo\nthree\nfour\nfive\nsix\n")
        .unwrap();
    f.git(&["add", "--", "a.txt"]).unwrap();
    f.commit_staged(10, "write it").unwrap();
    f.git(&["mv", "a.txt", "b.txt"]).unwrap();
    f.write_file("b.txt", "one\nTWO\nthree\nfour\nfive\nsix\n")
        .unwrap();
    f.git(&["add", "--", "b.txt"]).unwrap();
    let moved = f.commit_staged(11, "move and edit it").unwrap();

    let versions = open(&f).line_history("b.txt", "HEAD", 2, 50).unwrap();

    let at_move = versions
        .iter()
        .find(|version| version.oid == moved)
        .unwrap();
    assert_eq!(at_move.path, "b.txt", "{versions:#?}");
}
