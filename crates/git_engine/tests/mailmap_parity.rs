// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Every view shows the identity `git log --use-mailmap` shows: `.mailmap`, then
//! `mailmap.blob`, then `mailmap.file`, a later source winning.

use git_engine::{CommitQuery, RepoHandle};
use std::collections::{BTreeMap, BTreeSet};

type Ident = (String, String);

/// Held for the lifetime of the fixture: `mailmap.file` points into it.
struct Mapped {
    f: test_fixtures::Fixture,
    _outside: tempfile::TempDir,
}

/// One line per author, so blame and `-L` see every one of them.
const AUTHORS: &[&str] = &[
    // Name only, by address.
    "Ann Alias <alias@example.com>",
    // Two lines for one address: one gives the address, the other the name.
    "A. Author <old@example.com>",
    // Matched whatever the case, and the address keeps the case it was committed with.
    "Ann Author <Ann@Example.COM>",
    // By name and address.
    "Bob <BOB@example.com>",
    // Same address, another name: the by-name line does not apply.
    "Bob Other <bob@example.com>",
    // Only `mailmap.file` knows her.
    "Carol <carol@example.com>",
    // Only `mailmap.blob` knows him.
    "Dave <dave@example.com>",
    // All three sources name her; `mailmap.file` is read last.
    "Fay <fay@example.com>",
    // `.mailmap` gives the name, `mailmap.blob` the address.
    "Gus <gus@example.com>",
    "Eve <eve@example.com>",
];

const TREE_MAILMAP: &str = "\
# canonical identities
Ann Author <alias@example.com>
<ann@example.com> <old@example.com>
Ann Author <old@example.com>
Ann Proper <ann@example.com>
Robert <robert@example.com> Bob <bob@example.com>
Fixture Keeper <keeper@cogit.test> <fixture@cogit.test>
Fay Tree <fay@example.com>
Gus Tree <gus@example.com>
";

const BLOB_MAILMAP: &str = "\
David <david@example.com> <dave@example.com>
Fay Blob <fay@example.com>
<gus.blob@example.com> <gus@example.com>
";

const FILE_MAILMAP: &str = "\
Caroline <caroline@example.com> <carol@example.com>
Fay File <fay@example.com>
";

fn mapped() -> Mapped {
    let f = test_fixtures::Fixture::init().unwrap();
    f.write_file("people.mailmap", BLOB_MAILMAP).unwrap();
    f.git(&["add", "--", "people.mailmap"]).unwrap();
    f.commit_staged(1, "people").unwrap();

    let mut story = String::new();
    for (index, author) in AUTHORS.iter().enumerate() {
        story.push_str(author);
        story.push('\n');
        f.write_file("story.txt", &story).unwrap();
        f.git(&["add", "--", "story.txt"]).unwrap();
        f.git_at(
            10 + index as i64,
            &[
                "commit",
                "-m",
                &format!("line by {author}"),
                "--author",
                author,
            ],
        )
        .unwrap();
    }

    let outside = tempfile::TempDir::new().unwrap();
    let file = outside.path().join("people.mailmap");
    std::fs::write(&file, FILE_MAILMAP).unwrap();
    f.write_file(".mailmap", TREE_MAILMAP).unwrap();
    f.git(&["config", "mailmap.blob", "HEAD:people.mailmap"])
        .unwrap();
    f.git(&[
        "config",
        "mailmap.file",
        &file.to_string_lossy().replace('\\', "/"),
    ])
    .unwrap();
    Mapped {
        f,
        _outside: outside,
    }
}

fn open(m: &Mapped) -> RepoHandle {
    RepoHandle::open(m.f.path()).unwrap()
}

/// Commit id to (author, committer), as git prints them through the mailmap.
fn git_log(m: &Mapped) -> BTreeMap<String, (Ident, Ident)> {
    m.f.git(&[
        "log",
        "--use-mailmap",
        "--all",
        "--format=%H%x09%aN%x09%aE%x09%cN%x09%cE",
    ])
    .unwrap()
    .lines()
    .map(|line| {
        let f: Vec<&str> = line.split('\t').collect();
        (
            f[0].to_owned(),
            (
                (f[1].to_owned(), f[2].to_owned()),
                (f[3].to_owned(), f[4].to_owned()),
            ),
        )
    })
    .collect()
}

fn git_authors(m: &Mapped) -> BTreeMap<String, Ident> {
    git_log(m)
        .into_iter()
        .map(|(oid, (author, _))| (oid, author))
        .collect()
}

#[test]
fn the_fixture_really_maps_every_kind_of_line() {
    let m = mapped();
    let shown: BTreeSet<Ident> = git_authors(&m).into_values().collect();

    for expected in [
        ("Ann Author", "alias@example.com"),
        ("Ann Author", "ann@example.com"),
        ("Ann Proper", "Ann@Example.COM"),
        ("Robert", "robert@example.com"),
        ("Bob Other", "bob@example.com"),
        ("Caroline", "caroline@example.com"),
        ("David", "david@example.com"),
        ("Fay File", "fay@example.com"),
        ("Gus Tree", "gus.blob@example.com"),
        ("Eve", "eve@example.com"),
    ] {
        assert!(
            shown.contains(&(expected.0.to_owned(), expected.1.to_owned())),
            "git itself does not show {expected:?}: {shown:?}"
        );
    }
}

#[test]
fn graph_rows_show_the_authors_git_log_shows() {
    let m = mapped();
    let mut rows = BTreeMap::new();
    open(&m)
        .search_commits(&CommitQuery::default(), 100, |chunk| {
            for row in chunk {
                rows.insert(row.oid, (row.author_name, row.author_email));
            }
            true
        })
        .unwrap();

    assert_eq!(rows, git_authors(&m));
}

#[test]
fn commit_details_show_the_author_and_committer_git_log_shows() {
    let m = mapped();
    let repo = open(&m);

    for (oid, (author, committer)) in git_log(&m) {
        let details = repo.commit_details(&oid).unwrap();
        assert_eq!(
            (details.author.name, details.author.email),
            author,
            "author of {oid}"
        );
        assert_eq!(
            (details.committer.name, details.committer.email),
            committer,
            "committer of {oid}"
        );
    }
}

#[test]
fn blame_shows_the_authors_git_log_shows() {
    let m = mapped();
    let git = git_authors(&m);

    for line in open(&m).blame("story.txt", "HEAD").unwrap() {
        assert_eq!(
            (line.author, line.email),
            git[&line.oid],
            "line {}",
            line.line
        );
    }
}

#[test]
fn blame_with_origins_shows_the_authors_git_log_shows() {
    let m = mapped();
    let git = git_authors(&m);

    let report = open(&m)
        .blame_origins("story.txt", Some("HEAD"), false)
        .unwrap();
    assert!(!report.commits.is_empty());
    for commit in report.commits {
        assert_eq!((commit.author, commit.email), git[&commit.oid]);
    }
}

#[test]
fn investigate_shows_the_authors_git_log_shows() {
    let m = mapped();
    let git = git_authors(&m);
    let lines = u32::try_from(AUTHORS.len()).unwrap();

    let steps = open(&m).investigate("story.txt", 1, lines, 100).unwrap();
    assert_eq!(steps.len(), AUTHORS.len());
    for step in steps {
        assert_eq!((step.author, step.email), git[&step.oid]);
    }
}

#[test]
fn line_history_shows_the_authors_git_log_shows() {
    let m = mapped();
    let git = git_authors(&m);

    let versions = open(&m).line_history("story.txt", "HEAD", 1, 100).unwrap();
    assert!(!versions.is_empty());
    for version in versions {
        assert_eq!((version.author, version.email), git[&version.oid]);
    }
}

#[test]
fn file_log_shows_the_authors_git_log_shows() {
    let m = mapped();
    let git = git_authors(&m);

    let revisions = open(&m).file_log("story.txt", None, true, 100).unwrap();
    assert_eq!(revisions.len(), AUTHORS.len());
    for revision in revisions {
        assert_eq!((revision.author, revision.email), git[&revision.oid]);
    }
}

#[test]
fn searching_by_author_finds_what_git_log_author_finds() {
    let m = mapped();
    let repo = open(&m);

    for needle in ["robert", "bob", "alias", "ann proper", "caroline", "carol@"] {
        let git: BTreeSet<String> =
            m.f.git(&[
                "log",
                "--use-mailmap",
                "--all",
                "--regexp-ignore-case",
                "--fixed-strings",
                &format!("--author={needle}"),
                "--format=%H",
            ])
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect();

        let mut found = BTreeSet::new();
        let query = CommitQuery {
            author: Some(needle.to_owned()),
            ..CommitQuery::default()
        };
        repo.search_commits(&query, 100, |chunk| {
            found.extend(chunk.into_iter().map(|row| row.oid));
            true
        })
        .unwrap();

        assert_eq!(found, git, "--author={needle}");
    }
}

#[test]
fn one_person_under_two_addresses_is_one_address_to_fetch_a_picture_by() {
    let m = mapped();
    let mut keys = BTreeSet::new();
    open(&m)
        .search_commits(&CommitQuery::default(), 100, |chunk| {
            for row in chunk {
                if row.summary.contains("<old@") || row.summary.contains("<Ann@") {
                    // The key the avatar cache and the frontend both use.
                    keys.insert(row.author_email.trim().to_lowercase());
                }
            }
            true
        })
        .unwrap();

    assert_eq!(keys, BTreeSet::from(["ann@example.com".to_owned()]));
}
