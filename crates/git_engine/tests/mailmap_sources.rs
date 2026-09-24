// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Where the mailmap comes from, against `git check-mailmap`, and how fast an edit to
//! any of its sources shows through a handle that stays open.

use git_engine::RepoHandle;
use std::path::Path;

const PROBES: &[&str] = &[
    "Ann Alias <alias@example.com>",
    "A. Author <old@example.com>",
    "Ann Author <Ann@Example.COM>",
    "Bob <BOB@example.com>",
    "BOB <bob@example.com>",
    "Bob Other <bob@example.com>",
    "Carol <carol@example.com>",
    "Dave <dave@example.com>",
    "Fay <fay@example.com>",
    "Gus <gus@example.com>",
    "Eve <eve@example.com>",
];

const TREE_MAILMAP: &str = "\
Ann Author <alias@example.com>
<ann@example.com> <old@example.com>
Ann Author <old@example.com>
Ann Proper <ann@example.com>
Robert <robert@example.com> Bob <bob@example.com>
Fay Tree <fay@example.com>
Gus Tree <gus@example.com>
this line is not a mapping
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

fn git_check(f: &test_fixtures::Fixture, cwd: &Path, ident: &str) -> String {
    f.git_in(cwd, &["check-mailmap", ident])
        .unwrap()
        .trim_end()
        .to_owned()
}

fn ours(repo: &RepoHandle, ident: &str) -> String {
    let (name, rest) = ident.split_once(" <").unwrap();
    let email = rest.trim_end_matches('>');
    let map = repo.mailmap();
    let (name, email) = map.canonical(name, email);
    format!("{name} <{email}>")
}

fn assert_like_git(f: &test_fixtures::Fixture, cwd: &Path, repo: &RepoHandle) {
    for probe in PROBES {
        assert_eq!(
            ours(repo, probe),
            git_check(f, cwd, probe),
            "check-mailmap {probe}"
        );
    }
}

/// `.mailmap`, `mailmap.blob` and `mailmap.file` all set; the file sits outside the tree.
fn all_sources() -> (test_fixtures::Fixture, tempfile::TempDir) {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("people.mailmap", BLOB_MAILMAP).unwrap();
    f.git(&["add", "--", "people.mailmap"]).unwrap();
    f.commit_staged(5, "people").unwrap();
    f.write_file(".mailmap", TREE_MAILMAP).unwrap();

    let outside = tempfile::TempDir::new().unwrap();
    let file = outside.path().join("extra.mailmap");
    std::fs::write(&file, FILE_MAILMAP).unwrap();
    f.git(&["config", "mailmap.blob", "HEAD:people.mailmap"])
        .unwrap();
    f.git(&[
        "config",
        "mailmap.file",
        &file.to_string_lossy().replace('\\', "/"),
    ])
    .unwrap();
    (f, outside)
}

fn name_of_head_author(repo: &RepoHandle) -> String {
    repo.commit_details("HEAD").unwrap().author.name
}

#[test]
fn every_source_resolves_as_git_check_mailmap_does() {
    let (f, _outside) = all_sources();
    let repo = RepoHandle::open(f.path()).unwrap();

    assert_like_git(&f, f.path(), &repo);
}

#[test]
fn a_relative_mailmap_file_is_read_from_the_repository_root() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("people/extra.mailmap", FILE_MAILMAP).unwrap();
    f.git(&["config", "mailmap.file", "people/extra.mailmap"])
        .unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    assert_eq!(
        ours(&repo, "Carol <carol@example.com>"),
        "Caroline <caroline@example.com>"
    );
    assert_like_git(&f, f.path(), &repo);
}

#[test]
fn a_bare_repository_reads_the_mailmap_at_head() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file(".mailmap", TREE_MAILMAP).unwrap();
    f.git(&["add", "--", ".mailmap"]).unwrap();
    f.commit_staged(5, "mailmap").unwrap();
    let bare = tempfile::TempDir::new().unwrap();
    let target = bare.path().join("bare.git");
    f.git(&[
        "clone",
        "--bare",
        "--quiet",
        &f.path().to_string_lossy(),
        &target.to_string_lossy(),
    ])
    .unwrap();
    let repo = RepoHandle::open(&target).unwrap();

    assert_eq!(
        ours(&repo, "Ann Alias <alias@example.com>"),
        "Ann Author <alias@example.com>"
    );
    assert_like_git(&f, &target, &repo);
}

#[test]
fn a_mailmap_written_after_the_handle_opened_is_seen() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    let before = name_of_head_author(&repo);

    f.write_file(
        ".mailmap",
        &format!("Keeper <{}>\n", test_fixtures::AUTHOR_EMAIL),
    )
    .unwrap();

    assert_eq!(before, test_fixtures::AUTHOR_NAME);
    assert_eq!(name_of_head_author(&repo), "Keeper");
}

#[test]
fn an_edited_mailmap_is_seen_by_the_same_handle() {
    let f = test_fixtures::linear(1).unwrap();
    let email = test_fixtures::AUTHOR_EMAIL;
    f.write_file(".mailmap", &format!("First <{email}>\n"))
        .unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    assert_eq!(name_of_head_author(&repo), "First");

    f.write_file(".mailmap", &format!("Second Name <{email}>\n"))
        .unwrap();
    assert_eq!(name_of_head_author(&repo), "Second Name");

    std::fs::remove_file(f.path().join(".mailmap")).unwrap();
    assert_eq!(name_of_head_author(&repo), test_fixtures::AUTHOR_NAME);
}

#[test]
fn an_edited_mailmap_file_is_seen_by_the_same_handle() {
    let (f, outside) = all_sources();
    let repo = RepoHandle::open(f.path()).unwrap();
    assert_eq!(
        ours(&repo, "Carol <carol@example.com>"),
        "Caroline <caroline@example.com>"
    );

    std::fs::write(
        outside.path().join("extra.mailmap"),
        "Carol Changed <carol@example.com>\n",
    )
    .unwrap();

    assert_eq!(
        ours(&repo, "Carol <carol@example.com>"),
        "Carol Changed <carol@example.com>"
    );
}

#[test]
fn a_new_mailmap_blob_at_head_is_seen_by_the_same_handle() {
    let (f, _outside) = all_sources();
    let repo = RepoHandle::open(f.path()).unwrap();
    assert_eq!(
        ours(&repo, "Dave <dave@example.com>"),
        "David <david@example.com>"
    );

    f.write_file(
        "people.mailmap",
        "Davey <davey@example.com> <dave@example.com>\n",
    )
    .unwrap();
    f.git(&["add", "--", "people.mailmap"]).unwrap();
    f.commit_staged(6, "people again").unwrap();

    assert_eq!(
        ours(&repo, "Dave <dave@example.com>"),
        "Davey <davey@example.com>"
    );
}
