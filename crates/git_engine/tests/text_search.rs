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
    notes: false,
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
    assert!(fields.notes);
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

fn note(f: &Fixture, reference: &str, rev: &str, text: &str) {
    let oid = f.oid(rev).unwrap();
    f.git(&["notes", "--ref", reference, "add", "-m", text, oid.trim()])
        .unwrap();
}

#[test]
fn notes_are_on_by_default() {
    let f = history();
    note(&f, "commits", "HEAD~2", "remember ostrich");
    assert!(TextFields::default().notes);
    assert_eq!(
        found(&f, "ostrich", TextFields::default()),
        ["tune the parser"]
    );
}

#[test]
fn notes_match_in_any_namespace_ignoring_case() {
    let f = history();
    note(&f, "commits", "HEAD~2", "remember Ostrich");
    note(&f, "review", "HEAD", "needs QUUXX work");
    let notes = TextFields {
        notes: true,
        ..NONE
    };
    assert_eq!(found(&f, "ostrich", notes), ["tune the parser"]);
    assert_eq!(found(&f, "quuxx", notes), ["touch the readme"]);
    assert!(found(&f, "absent", notes).is_empty());
}

#[test]
fn notes_do_not_match_when_the_switch_is_off() {
    let f = history();
    note(&f, "review", "HEAD", "needs QUUXX work");
    let message = TextFields {
        message: true,
        ..NONE
    };
    assert!(found(&f, "quuxx", message).is_empty());
}

#[test]
fn a_repo_without_notes_matches_nothing_in_notes() {
    let f = history();
    let notes = TextFields {
        notes: true,
        ..NONE
    };
    assert!(found(&f, "the", notes).is_empty());
}

/// Past one batch of the parallel scan: order and results are those of a one-by-one walk.
#[test]
fn a_long_history_matches_name_and_content_in_walk_order() {
    let f = test_fixtures::linear(700).unwrap();
    let name = TextFields { name: true, ..NONE };
    let content = TextFields {
        content: true,
        ..NONE
    };
    let expected: Vec<String> = (0..700)
        .rev()
        .filter(|i| i.to_string().starts_with("69"))
        .map(|i| format!("commit {i}"))
        .collect();
    assert_eq!(found(&f, "content 69", content), expected);
    let expected: Vec<String> = (0..700)
        .rev()
        .filter(|i| format!("file{i}.txt").contains("file6"))
        .map(|i| format!("commit {i}"))
        .collect();
    assert_eq!(found(&f, "file6", name), expected);
    assert!(found(&f, "absent", name).is_empty());
}

#[test]
fn a_long_history_stops_when_the_consumer_does() {
    let f = test_fixtures::linear(700).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    let query = CommitQuery {
        text: Some("content".to_owned()),
        text_in: TextFields {
            content: true,
            ..NONE
        },
        ..CommitQuery::default()
    };
    let mut chunks = 0;
    repo.search_commits(&query, 10, |_| {
        chunks += 1;
        false
    })
    .unwrap();
    assert_eq!(chunks, 1);
}

/// \`history()\` plus a side branch merged back, a binary file, CRLF text, non-ASCII names and
/// text, a deleted and a moved file.
fn messy() -> Fixture {
    let f = history();
    f.git(&["checkout", "-q", "-b", "side"]).unwrap();
    f.write_file("Ünï/Fïle.TXT", "ÄÖÜ Straße\r\nsecond\r\n")
        .unwrap();
    f.git(&["add", "--", "Ünï/Fïle.TXT"]).unwrap();
    f.commit_staged(10, "unicode ÄRGER").unwrap();
    std::fs::write(f.path().join("blob.bin"), b"\0needle in binary\n").unwrap();
    f.git(&["add", "--", "blob.bin"]).unwrap();
    f.commit_staged(11, "binary needle").unwrap();
    f.git(&["checkout", "-q", "main"]).unwrap();
    f.write_file("README.md", "hello there\nsecond line\nNEEDLE tail\n")
        .unwrap();
    f.git(&["add", "--", "README.md"]).unwrap();
    f.commit_staged(12, "readme tail").unwrap();
    f.merge(13, &["side"], "merge side into master").unwrap();
    f.git(&["rm", "-q", "--", "src/widget.rs"]).unwrap();
    f.commit_staged(14, "drop the widget").unwrap();
    f.git(&["mv", "src/parser.rs", "src/lexer.rs"]).unwrap();
    f.commit_staged(15, "move parser").unwrap();
    f.write_file("README.md", "second line\nhello there\nNEEDLE tail\n")
        .unwrap();
    f.git(&["add", "--", "README.md"]).unwrap();
    f.commit_staged(16, "shuffle lines").unwrap();
    note(&f, "commits", "HEAD~3", "note about Straße");
    f.git(&["tag", "-a", "-m", "annotated", "rel/1"]).unwrap();
    f
}

#[test]
fn a_messy_history_matches_as_it_always_did() {
    let f = messy();
    let one = |field: &str| TextFields {
        author: field == "author",
        committer: field == "committer",
        message: field == "message",
        refs: field == "refs",
        id: field == "id",
        name: field == "name",
        content: field == "content",
        notes: field == "notes",
    };
    let fields = [
        "author",
        "committer",
        "message",
        "refs",
        "id",
        "name",
        "content",
        "notes",
    ];
    let needles = [
        "needle",
        "STRASSE",
        "straße",
        "ÄRGER",
        "ärger",
        "ünï",
        "fïle.txt",
        "src/",
        "lexer",
        "carol",
        "fixture",
        "rel/",
        "second line",
        "shuffle",
        "hello",
        "zzz",
    ];
    let mut got = String::new();
    for needle in needles {
        for field in fields {
            let rows = found(&f, needle, one(field));
            got.push_str(&format!("{needle}|{field}|{}\n", rows.join(",")));
        }
        let all = TextFields {
            name: true,
            content: true,
            ..TextFields::default()
        };
        got.push_str(&format!(
            "{needle}|all|{}\n",
            found(&f, needle, all).join(",")
        ));
    }
    assert_eq!(
        got,
        include_str!("text_search_golden.txt").replace("\r\n", "\n")
    );
}
