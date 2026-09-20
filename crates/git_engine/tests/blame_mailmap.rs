// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! `.mailmap` folds one person's several spellings into the one they want shown.

use git_engine::RepoHandle;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

/// Three lines from one person committing under three different identities.
fn many_spellings() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(1).unwrap();

    for (index, (line, author)) in [
        ("one\n", "Ann Alias <alias@example.com>"),
        ("one\ntwo\n", "A. Author <old@example.com>"),
        ("one\ntwo\nthree\n", "Ann Author <ann@example.com>"),
    ]
    .into_iter()
    .enumerate()
    {
        f.write_file("story.txt", line).unwrap();
        f.git(&["add", "--", "story.txt"]).unwrap();
        f.git_at(
            10 + index as i64,
            &["commit", "-m", "a line", "--author", author],
        )
        .unwrap();
    }
    f
}

fn authors(f: &test_fixtures::Fixture) -> Vec<(String, String)> {
    open(f)
        .blame("story.txt", "HEAD")
        .unwrap()
        .into_iter()
        .map(|line| (line.author, line.email))
        .collect()
}

#[test]
fn without_a_mailmap_every_spelling_stands_as_committed() {
    let f = many_spellings();

    assert_eq!(
        authors(&f),
        [
            ("Ann Alias".to_owned(), "alias@example.com".to_owned()),
            ("A. Author".to_owned(), "old@example.com".to_owned()),
            ("Ann Author".to_owned(), "ann@example.com".to_owned()),
        ]
    );
}

#[test]
fn two_spellings_of_one_author_collapse_to_the_canonical_name() {
    let f = many_spellings();
    f.write_file(
        ".mailmap",
        "Ann Author <ann@example.com> <alias@example.com>\n\
         Ann Author <ann@example.com> <old@example.com>\n",
    )
    .unwrap();

    let seen = authors(&f);

    assert!(
        seen.iter()
            .all(|(name, email)| name == "Ann Author" && email == "ann@example.com"),
        "{seen:?}"
    );
}

#[test]
fn a_mailmap_can_rewrite_the_name_while_keeping_the_address() {
    let f = many_spellings();
    f.write_file(".mailmap", "Ann Author <alias@example.com>\n")
        .unwrap();

    let seen = authors(&f);

    assert_eq!(
        seen[0],
        ("Ann Author".to_owned(), "alias@example.com".to_owned())
    );
}

#[test]
fn an_address_the_mailmap_does_not_mention_is_left_alone() {
    let f = many_spellings();
    f.write_file(
        ".mailmap",
        "Ann Author <ann@example.com> <alias@example.com>\n",
    )
    .unwrap();

    let seen = authors(&f);

    assert_eq!(
        seen[1],
        ("A. Author".to_owned(), "old@example.com".to_owned())
    );
}

#[test]
fn a_repository_without_a_mailmap_is_not_an_error() {
    let f = many_spellings();

    assert_eq!(authors(&f).len(), 3);
}

#[test]
fn an_unparsable_mailmap_does_not_break_blame() {
    let f = many_spellings();
    f.write_file(".mailmap", "this is not a mailmap line at all\n<<<>>>\n")
        .unwrap();

    assert_eq!(
        authors(&f).len(),
        3,
        "a broken .mailmap must not cost the user their blame"
    );
}
