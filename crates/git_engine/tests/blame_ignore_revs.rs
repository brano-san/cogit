// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! `.git-blame-ignore-revs` keeps a reformatting commit from owning every line it touched.

use git_engine::RepoHandle;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

struct Story {
    fixture: test_fixtures::Fixture,
    wrote: String,
    reindented: String,
}

/// One commit writes the lines, the next only re-indents them.
fn reindented() -> Story {
    let f = test_fixtures::linear(1).unwrap();

    f.write_file("story.txt", "one\ntwo\nthree\n").unwrap();
    f.git(&["add", "--", "story.txt"]).unwrap();
    let wrote = f.commit_staged(10, "write the lines").unwrap();

    f.write_file("story.txt", "    one\n    two\n    three\n")
        .unwrap();
    f.git(&["add", "--", "story.txt"]).unwrap();
    let reindented = f.commit_staged(11, "reindent everything").unwrap();

    Story {
        fixture: f,
        wrote,
        reindented,
    }
}

fn blamed_to(f: &test_fixtures::Fixture) -> Vec<String> {
    open(f)
        .blame("story.txt", "HEAD")
        .unwrap()
        .into_iter()
        .map(|line| line.oid)
        .collect()
}

#[test]
fn without_the_file_the_reindent_owns_every_line() {
    let story = reindented();

    let owners = blamed_to(&story.fixture);

    assert!(
        owners.iter().all(|oid| *oid == story.reindented),
        "baseline: the cosmetic commit owns the lines"
    );
}

#[test]
fn a_cosmetic_commit_listed_in_the_ignore_file_is_skipped() {
    let story = reindented();
    story
        .fixture
        .write_file(".git-blame-ignore-revs", &format!("{}\n", story.reindented))
        .unwrap();

    let owners = blamed_to(&story.fixture);

    assert!(
        owners.iter().all(|oid| *oid == story.wrote),
        "the lines belong to whoever wrote them: {owners:?}"
    );
}

#[test]
fn comments_and_blank_lines_in_the_ignore_file_are_skipped() {
    let story = reindented();
    story
        .fixture
        .write_file(
            ".git-blame-ignore-revs",
            &format!(
                "# a reformatting pass\n\n  {}  \n\n# end\n",
                story.reindented
            ),
        )
        .unwrap();

    let owners = blamed_to(&story.fixture);

    assert!(owners.iter().all(|oid| *oid == story.wrote), "{owners:?}");
}

#[test]
fn a_missing_ignore_file_is_not_an_error() {
    let story = reindented();

    assert_eq!(blamed_to(&story.fixture).len(), 3);
}

#[test]
fn an_unparsable_line_in_the_ignore_file_does_not_break_blame() {
    let story = reindented();
    story
        .fixture
        .write_file(
            ".git-blame-ignore-revs",
            &format!("not-a-hash\nzzzz\n{}\n", story.reindented),
        )
        .unwrap();

    let owners = blamed_to(&story.fixture);

    assert!(
        owners.iter().all(|oid| *oid == story.wrote),
        "garbage lines must be skipped, not fatal: {owners:?}"
    );
}

#[test]
fn an_ignored_commit_that_never_touched_the_file_changes_nothing() {
    let story = reindented();
    let unrelated = story.fixture.oid("HEAD~2").unwrap();
    story
        .fixture
        .write_file(".git-blame-ignore-revs", &format!("{unrelated}\n"))
        .unwrap();

    let owners = blamed_to(&story.fixture);

    assert!(
        owners.iter().all(|oid| *oid == story.reindented),
        "{owners:?}"
    );
}
