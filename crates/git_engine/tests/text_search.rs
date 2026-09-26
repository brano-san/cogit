// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The graph filter's free text, matched in the fields its switches pick (F-560).

use git_engine::{CommitQuery, CommitRow, RepoHandle, TextFields};
use test_fixtures::Fixture;

const NONE: TextFields = TextFields {
    author: false,
    committer: false,
    message: false,
    refs: false,
    id: false,
    name: false,
    content: false,
};

/// Four commits, newest last: the second has another committer and a body, the third a
/// branch and a tag, the fourth changes the line the first added.
fn history() -> Fixture {
    let f = Fixture::init().unwrap();
    f.commit_file(0, "README.md", "hello world\nsecond line\n")
        .unwrap();

    // The fixture's environment names one committer for every commit, so this one is
    // written by hand.
    f.write_file("src/parser.rs", "fn parse() {}\n").unwrap();
    f.git(&["add", "--", "src/parser.rs"]).unwrap();
    let tree = f.git(&["write-tree"]).unwrap();
    let parent = f.oid("HEAD").unwrap();
    let object = f.git_dir().join("carol-commit");
    std::fs::write(
        &object,
        format!(
            "tree {}\nparent {}\nauthor Cogit Fixture <fixture@cogit.test> 1767225660 +0000\n\
             committer Carol Committer <carol@example.test> 1767225660 +0000\n\n\
             tune the parser\n\nMentions quux in the body.\n",
            tree.trim(),
            parent.trim()
        ),
    )
    .unwrap();
    let written = f
        .git(&[
            "hash-object",
            "-t",
            "commit",
            "-w",
            &object.to_string_lossy(),
        ])
        .unwrap();
    f.git(&["reset", "--soft", written.trim()]).unwrap();

    f.write_file("src/widget.rs", "let gadget = 1;\n").unwrap();
    f.git(&["add", "--", "src/widget.rs"]).unwrap();
    f.commit_staged(2, "add the third file").unwrap();
    f.git(&["branch", "feature/zeta"]).unwrap();
    f.git(&["tag", "v1.0"]).unwrap();

    f.write_file("README.md", "hello there\nsecond line\n")
        .unwrap();
    f.git(&["add", "--", "README.md"]).unwrap();
    f.commit_staged(3, "touch the readme").unwrap();
    f
}

fn found(f: &Fixture, text: &str, fields: TextFields) -> Vec<String> {
    let repo = RepoHandle::open(f.path()).unwrap();
    let query = CommitQuery {
        text: Some(text.to_owned()),
        text_in: fields,
        ..CommitQuery::default()
    };
    let mut rows: Vec<CommitRow> = Vec::new();
    repo.search_commits(&query, 100, |chunk| {
        rows.extend(chunk);
        true
    })
    .unwrap();
    rows.into_iter().map(|row| row.summary).collect()
}

#[test]
fn the_default_fields_are_all_but_name_and_content() {
    let fields = TextFields::default();
    assert!(fields.author && fields.committer && fields.message && fields.refs && fields.id);
    assert!(!fields.name && !fields.content);
}

#[test]
fn the_message_is_searched_whole_body_included_and_ignoring_case() {
    let f = history();
    let message = TextFields {
        message: true,
        ..NONE
    };
    assert_eq!(found(&f, "QUUX", message), ["tune the parser"]);
}

#[test]
fn the_committer_is_searched_apart_from_the_author() {
    let f = history();
    let committer = TextFields {
        committer: true,
        ..NONE
    };
    let author = TextFields {
        author: true,
        ..NONE
    };
    assert_eq!(found(&f, "carol", committer), ["tune the parser"]);
    assert!(found(&f, "carol", author).is_empty());
    assert_eq!(found(&f, "example.test", committer), ["tune the parser"]);
}

#[test]
fn a_ref_name_finds_the_commit_it_points_at() {
    let f = history();
    let refs = TextFields { refs: true, ..NONE };
    assert_eq!(found(&f, "zeta", refs), ["add the third file"]);
    assert_eq!(found(&f, "v1.", refs), ["add the third file"]);
}

#[test]
fn an_id_prefix_finds_its_commit() {
    let f = history();
    let oid = f.oid("HEAD~2").unwrap();
    let id = TextFields { id: true, ..NONE };
    assert_eq!(found(&f, &oid[..8].to_uppercase(), id), ["tune the parser"]);
}

#[test]
fn a_field_switched_off_finds_nothing() {
    let f = history();
    let everything_but_committer = TextFields {
        committer: false,
        ..TextFields::default()
    };
    assert!(found(&f, "carol", everything_but_committer).is_empty());
}

#[test]
fn any_field_that_matches_is_enough() {
    let f = history();
    let mut summaries = found(&f, "the", TextFields::default());
    summaries.sort();
    assert_eq!(
        summaries,
        ["add the third file", "touch the readme", "tune the parser"]
    );
}

#[test]
fn name_matches_the_file_names_the_commit_changed() {
    let f = history();
    let name = TextFields { name: true, ..NONE };
    assert_eq!(found(&f, "WIDGET", name), ["add the third file"]);
    assert!(
        found(&f, "src", name).is_empty(),
        "without a slash only the name counts"
    );
}

#[test]
fn a_slash_makes_name_match_the_whole_path() {
    let f = history();
    let name = TextFields { name: true, ..NONE };
    let mut summaries = found(&f, "src/", name);
    summaries.sort();
    assert_eq!(summaries, ["add the third file", "tune the parser"]);
}

#[test]
fn content_matches_a_line_the_commit_added_or_removed() {
    let f = history();
    let content = TextFields {
        content: true,
        ..NONE
    };
    let mut summaries = found(&f, "World", content);
    summaries.sort();
    assert_eq!(summaries, ["commit 0", "touch the readme"]);
    assert!(
        found(&f, "second line", content) == ["commit 0"],
        "a line the commit left alone is no change"
    );
}

#[test]
fn content_is_off_by_default() {
    let f = history();
    assert!(found(&f, "gadget", TextFields::default()).is_empty());
}

#[test]
fn switches_without_text_filter_nothing() {
    let query = CommitQuery {
        text_in: TextFields {
            content: true,
            name: true,
            ..TextFields::default()
        },
        ..CommitQuery::default()
    };
    assert!(query.is_empty());
    assert!(!query.filters_rows());
}

#[test]
fn a_commit_is_shown_by_the_text_it_matches() {
    let f = history();
    let repo = RepoHandle::open(f.path()).unwrap();
    let query = CommitQuery {
        text: Some("quux".to_owned()),
        ..CommitQuery::default()
    };
    assert!(repo.shown_by(&query, f.oid("HEAD~2").unwrap().trim()));
    assert!(!repo.shown_by(&query, f.oid("HEAD").unwrap().trim()));
}
