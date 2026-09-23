// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{ContentMatch, RepoHandle, SearchRequest, SearchScope};

fn open(fixture: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(fixture.path()).unwrap()
}

/// Collects everything the search finds, ignoring the batching.
fn find(repo: &RepoHandle, request: &SearchRequest) -> Vec<ContentMatch> {
    let mut found = Vec::new();
    repo.search_contents(request, &|| false, &mut |batch| found.extend(batch))
        .unwrap();
    found
}

fn literal(query: &str, scope: SearchScope) -> SearchRequest<'_> {
    SearchRequest {
        query,
        is_regex: false,
        scope,
    }
}

#[test]
fn every_tracked_file_is_listed() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);

    let files = repo.all_files().unwrap();

    assert!(files.contains(&"file0.txt".to_owned()), "{files:?}");
    assert!(files.contains(&"file2.txt".to_owned()), "{files:?}");
}

#[test]
fn an_untracked_file_is_listed_too() {
    // The whole point of the command: the panel must find a file that no change mentions.
    let f = test_fixtures::linear(2).unwrap();
    f.write_file("brand-new.txt", "hello\n").unwrap();
    let repo = open(&f);

    assert!(
        repo.all_files()
            .unwrap()
            .contains(&"brand-new.txt".to_owned())
    );
}

#[test]
fn an_ignored_file_is_not_listed() {
    let f = test_fixtures::linear(2).unwrap();
    f.write_file(".gitignore", "secret.txt\n").unwrap();
    f.write_file("secret.txt", "hush\n").unwrap();
    let repo = open(&f);

    let files = repo.all_files().unwrap();
    assert!(!files.contains(&"secret.txt".to_owned()), "{files:?}");
}

#[test]
fn the_list_has_no_duplicates() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);

    let mut files = repo.all_files().unwrap();
    let before = files.len();
    files.sort();
    files.dedup();

    assert_eq!(files.len(), before);
}

#[test]
fn a_literal_search_finds_the_line_it_is_on() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);

    let found = find(&repo, &literal("content 2", SearchScope::All));

    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].path, "file2.txt");
    assert_eq!(found[0].line, 1);
    assert!(found[0].preview.contains("content 2"));
}

#[test]
fn a_regex_search_is_a_regex() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);

    let found = find(
        &repo,
        &SearchRequest {
            query: r"content [02]",
            is_regex: true,
            scope: SearchScope::All,
        },
    );

    assert_eq!(found.len(), 2, "{found:?}");
}

#[test]
fn a_broken_regex_is_an_error_rather_than_no_results() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    let mut found = Vec::new();
    let result = repo.search_contents(
        &SearchRequest {
            query: "(unclosed",
            is_regex: true,
            scope: SearchScope::All,
        },
        &|| false,
        &mut |batch| found.extend(batch),
    );

    assert!(result.is_err(), "a query that cannot compile must say so");
}

#[test]
fn a_literal_query_is_not_read_as_a_regex() {
    let f = test_fixtures::linear(2).unwrap();
    f.write_file("dots.txt", "a.b\n").unwrap();
    let repo = open(&f);

    // As a regex `a.b` would also match `axb`; as a literal it must not.
    f.write_file("other.txt", "axb\n").unwrap();
    let found = find(&repo, &literal("a.b", SearchScope::All));

    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].path, "dots.txt");
}

#[test]
fn a_binary_file_is_never_scanned() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("blob.bin"), b"needle\0\0\0needle").unwrap();
    let repo = open(&f);

    let found = find(&repo, &literal("needle", SearchScope::All));

    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_file_over_the_cap_is_never_scanned() {
    let f = test_fixtures::linear(1).unwrap();
    let mut huge = String::with_capacity(3 * 1024 * 1024);
    while huge.len() < 3 * 1024 * 1024 {
        huge.push_str("padding padding padding\n");
    }
    huge.push_str("needle\n");
    std::fs::write(f.path().join("huge.txt"), huge).unwrap();
    let repo = open(&f);

    assert!(find(&repo, &literal("needle", SearchScope::All)).is_empty());
}

#[test]
fn a_long_line_is_cut_down_to_a_preview() {
    let f = test_fixtures::linear(1).unwrap();
    let long = format!("{}needle{}", "x".repeat(500), "y".repeat(500));
    f.write_file("long.txt", &format!("{long}\n")).unwrap();
    let repo = open(&f);

    let found = find(&repo, &literal("needle", SearchScope::All));

    assert_eq!(found.len(), 1);
    assert!(
        found[0].preview.chars().count() <= git_engine::PREVIEW_CHARS,
        "preview was {} chars",
        found[0].preview.chars().count()
    );
}

#[test]
fn the_changed_scope_looks_only_at_what_changed() {
    let f = test_fixtures::linear(3).unwrap();
    f.write_file("touched.txt", "needle\n").unwrap();
    let repo = open(&f);

    let changed = find(&repo, &literal("needle", SearchScope::Changed));
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].path, "touched.txt");
}

#[test]
fn a_cancelled_search_stops_early() {
    let f = test_fixtures::linear(50).unwrap();
    let repo = open(&f);

    let mut found = Vec::new();
    repo.search_contents(
        &literal("content", SearchScope::All),
        &|| true,
        &mut |batch| {
            found.extend(batch);
        },
    )
    .unwrap();

    assert!(
        found.is_empty(),
        "cancelled before the first file, found {found:?}"
    );
}

#[test]
fn results_arrive_in_batches_rather_than_one_lump() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);

    let mut batches = 0_usize;
    repo.search_contents(
        &literal("content", SearchScope::All),
        &|| false,
        &mut |batch| {
            assert!(!batch.is_empty(), "an empty batch says nothing");
            batches += 1;
        },
    )
    .unwrap();

    assert!(batches >= 1);
}

#[test]
fn a_commit_lists_every_file_of_its_tree_and_no_directories() {
    let f = test_fixtures::Fixture::init().unwrap();
    f.commit_file(0, "top.txt", "a\n").unwrap();
    let oid = f.commit_file(1, "src/deep/inner.rs", "b\n").unwrap();
    let repo = open(&f);

    let files = repo.tree_files(&oid).unwrap();

    assert_eq!(
        files,
        vec!["src/deep/inner.rs".to_owned(), "top.txt".to_owned()]
    );
}

#[test]
fn a_commit_lists_its_own_tree_rather_than_the_working_tree() {
    let f = test_fixtures::linear(3).unwrap();
    f.write_file("brand-new.txt", "hello\n").unwrap();
    std::fs::remove_file(f.path().join("file0.txt")).unwrap();
    let repo = open(&f);

    let first = repo.tree_files(&f.oid("HEAD~2").unwrap()).unwrap();
    let head = repo.tree_files("HEAD").unwrap();

    assert_eq!(first, vec!["file0.txt".to_owned()]);
    assert!(head.contains(&"file0.txt".to_owned()), "{head:?}");
    assert!(!head.contains(&"brand-new.txt".to_owned()), "{head:?}");
}

#[test]
fn a_submodule_is_listed_as_one_entry() {
    let f = test_fixtures::with_submodule().unwrap();
    let repo = open(&f);

    let files = repo.tree_files("HEAD").unwrap();

    assert!(files.contains(&"vendor/lib".to_owned()), "{files:?}");
    assert!(
        !files.iter().any(|path| path.starts_with("vendor/lib/")),
        "{files:?}"
    );
}
