// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::RepoHandle;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn text(side: Option<Vec<u8>>) -> String {
    String::from_utf8(side.expect("this side should exist")).unwrap()
}

/// A conflict where both sides changed the same line, plus a common ancestor to compare to.
fn three_sided() -> (test_fixtures::Fixture, String) {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("shared.txt", "base line\nkeep\n").unwrap();
    f.git(&["add", "--", "shared.txt"]).unwrap();
    f.commit_staged(10, "add shared.txt").unwrap();

    f.git(&["switch", "-c", "theirs"]).unwrap();
    f.write_file("shared.txt", "their line\nkeep\n").unwrap();
    f.git(&["add", "--", "shared.txt"]).unwrap();
    f.commit_staged(11, "their change").unwrap();

    f.git(&["switch", "main"]).unwrap();
    f.write_file("shared.txt", "our line\nkeep\n").unwrap();
    f.git(&["add", "--", "shared.txt"]).unwrap();
    f.commit_staged(12, "our change").unwrap();

    let _ = f.git(&["merge", "theirs"]);
    (f, "shared.txt".to_owned())
}

#[test]
fn all_three_sides_are_readable_while_the_merge_is_unresolved() {
    let (f, path) = three_sided();
    let repo = open(&f);

    let sides = repo.conflict_sides(&path).unwrap();

    assert_eq!(text(sides.base), "base line\nkeep\n");
    assert_eq!(text(sides.ours), "our line\nkeep\n");
    assert_eq!(text(sides.theirs), "their line\nkeep\n");
}

#[test]
fn the_working_copy_carries_the_markers_git_wrote() {
    let (f, path) = three_sided();

    let merged = std::fs::read_to_string(f.path().join(&path)).unwrap();

    assert!(merged.contains("<<<<<<<"), "{merged}");
    assert!(merged.contains(">>>>>>>"), "{merged}");
}

#[test]
fn a_file_that_is_not_conflicted_has_no_sides() {
    let f = test_fixtures::linear(2).unwrap();

    assert!(open(&f).conflict_sides("file0.txt").is_err());
}

#[test]
fn the_conflicted_paths_are_listed() {
    let (f, path) = three_sided();

    let paths = open(&f).conflicted_paths().unwrap();

    assert_eq!(paths, vec![path]);
}

#[test]
fn a_clean_repository_lists_no_conflicts() {
    let f = test_fixtures::linear(2).unwrap();
    assert!(open(&f).conflicted_paths().unwrap().is_empty());
}

#[test]
fn taking_our_side_resolves_the_file_with_our_content() {
    let (f, path) = three_sided();
    let repo = open(&f);

    repo.resolve_with(&path, git_engine::ConflictSide::Ours, None)
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(f.path().join(&path)).unwrap(),
        "our line\nkeep\n"
    );
    assert!(repo.conflicted_paths().unwrap().is_empty());
}

#[test]
fn taking_their_side_resolves_the_file_with_their_content() {
    let (f, path) = three_sided();
    let repo = open(&f);

    repo.resolve_with(&path, git_engine::ConflictSide::Theirs, None)
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(f.path().join(&path)).unwrap(),
        "their line\nkeep\n"
    );
}

/// Resolving with our own side leaves the content equal to HEAD, so there is no staged
/// *change* — what matters is that the conflict left the index and the merge can finish.
#[test]
fn a_resolved_file_lets_the_merge_finish() {
    let (f, path) = three_sided();
    let repo = open(&f);

    repo.resolve_with(&path, git_engine::ConflictSide::Ours, None)
        .unwrap();
    repo.continue_operation().unwrap();

    assert_eq!(repo.state().unwrap(), git_engine::RepoState::Clean);
    assert_eq!(repo.commit_details("HEAD").unwrap().parents.len(), 2);
}

#[test]
fn writing_a_hand_edited_resolution_stages_exactly_that_text() {
    let (f, path) = three_sided();
    let repo = open(&f);

    repo.resolve_with_text(&path, "merged by hand\nkeep\n", None)
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(f.path().join(&path)).unwrap(),
        "merged by hand\nkeep\n"
    );
    assert!(repo.conflicted_paths().unwrap().is_empty());
}

#[test]
fn resolving_a_path_that_is_not_conflicted_is_refused() {
    let f = test_fixtures::linear(2).unwrap();

    assert!(
        open(&f)
            .resolve_with("file0.txt", git_engine::ConflictSide::Ours, None)
            .is_err()
    );
}

/// `three_sided`, with every text file checked out with CRLF.
fn three_sided_crlf() -> (test_fixtures::Fixture, String) {
    let f = test_fixtures::empty().unwrap();
    f.write_file(".gitattributes", "*.txt text eol=crlf\n")
        .unwrap();
    f.write_file("shared.txt", "base line\nkeep\n").unwrap();
    f.git(&["add", "--", ".gitattributes", "shared.txt"])
        .unwrap();
    f.commit_staged(10, "add shared.txt").unwrap();

    f.git(&["switch", "-c", "theirs"]).unwrap();
    f.write_file("shared.txt", "their line\r\nkeep\r\n")
        .unwrap();
    f.git(&["add", "--", "shared.txt"]).unwrap();
    f.commit_staged(11, "their change").unwrap();

    f.git(&["switch", "main"]).unwrap();
    f.write_file("shared.txt", "our line\r\nkeep\r\n").unwrap();
    f.git(&["add", "--", "shared.txt"]).unwrap();
    f.commit_staged(12, "our change").unwrap();

    let _ = f.git(&["merge", "theirs"]);
    (f, "shared.txt".to_owned())
}

// The stage's raw blob went to disk past the eol and smudge filters: LF where the
// attributes ask for CRLF, and the text of an LFS pointer instead of the file.
#[test]
fn taking_a_side_writes_the_file_as_a_checkout_would() {
    let (f, path) = three_sided_crlf();
    let repo = open(&f);

    repo.resolve_with(&path, git_engine::ConflictSide::Theirs, None)
        .unwrap();

    assert_eq!(
        std::fs::read(f.path().join(&path)).unwrap(),
        b"their line\r\nkeep\r\n"
    );
    assert_eq!(
        f.git(&["show", ":shared.txt"]).unwrap(),
        "their line\nkeep\n"
    );
    assert!(repo.conflicted_paths().unwrap().is_empty());
}

#[test]
fn taking_the_base_writes_it_as_a_checkout_would() {
    let (f, path) = three_sided_crlf();
    let repo = open(&f);

    repo.resolve_with(&path, git_engine::ConflictSide::Base, None)
        .unwrap();

    assert_eq!(
        std::fs::read(f.path().join(&path)).unwrap(),
        b"base line\r\nkeep\r\n"
    );
    assert!(repo.conflicted_paths().unwrap().is_empty());
}

/// A conflict whose three sides are exactly these bytes, committed as they are.
fn conflict_of(base: &[u8], ours: &[u8], theirs: &[u8]) -> (test_fixtures::Fixture, String) {
    let f = test_fixtures::empty().unwrap();
    let commit = |bytes: &[u8], index: i64| {
        std::fs::write(f.path().join("f.txt"), bytes).unwrap();
        f.git(&["add", "--", "f.txt"]).unwrap();
        f.commit_staged(index, "side").unwrap();
    };
    commit(base, 10);
    f.git(&["switch", "-c", "theirs"]).unwrap();
    commit(theirs, 11);
    f.git(&["switch", "main"]).unwrap();
    commit(ours, 12);
    let _ = f.git(&["merge", "theirs"]);
    (f, "f.txt".to_owned())
}

// The merge view hands over its result joined with LF and ending in a newline: a CRLF file
// was committed with every line changed.
#[test]
fn a_text_resolution_keeps_the_line_endings_of_our_side() {
    let (f, path) = conflict_of(
        b"base\r\nkeep\r\n",
        b"ours\r\nkeep\r\n",
        b"theirs\r\nkeep\r\n",
    );
    let repo = open(&f);

    repo.resolve_with_text(&path, "merged\nkeep\n", None)
        .unwrap();

    assert_eq!(
        std::fs::read(f.path().join(&path)).unwrap(),
        b"merged\r\nkeep\r\n"
    );
    assert_eq!(f.git(&["show", ":f.txt"]).unwrap(), "merged\r\nkeep\r\n");
}

#[test]
fn a_text_resolution_keeps_our_side_without_a_final_newline() {
    let (f, path) = conflict_of(b"base\nkeep", b"ours\nkeep", b"theirs\nkeep");
    let repo = open(&f);

    repo.resolve_with_text(&path, "merged\nkeep\n", None)
        .unwrap();

    assert_eq!(f.git(&["show", ":f.txt"]).unwrap(), "merged\nkeep");
}

#[test]
fn a_text_resolution_is_checked_out_as_the_attributes_ask() {
    let (f, path) = three_sided_crlf();
    let repo = open(&f);

    repo.resolve_with_text(&path, "merged\nkeep\n", None)
        .unwrap();

    assert_eq!(
        std::fs::read(f.path().join(&path)).unwrap(),
        b"merged\r\nkeep\r\n"
    );
    assert_eq!(f.git(&["show", ":shared.txt"]).unwrap(), "merged\nkeep\n");
}

/// Two branches point `path` at different entries of mode `mode` (a symlink or a gitlink).
/// Written through the index, so it needs neither `core.symlinks` nor a real submodule.
fn special_conflict(mode: &str, path: &str, oids: [&str; 3]) -> test_fixtures::Fixture {
    let f = test_fixtures::linear(1).unwrap();
    let point = |oid: &str| {
        let id = if mode == "120000" {
            f.write_file("target.tmp", oid).unwrap();
            let id = f.git(&["hash-object", "-w", "target.tmp"]).unwrap();
            std::fs::remove_file(f.path().join("target.tmp")).unwrap();
            id.trim().to_owned()
        } else {
            oid.to_owned()
        };
        f.git(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("{mode},{id},{path}"),
        ])
        .unwrap();
    };
    point(oids[0]);
    f.commit_staged(20, "base").unwrap();
    f.git(&["switch", "-c", "theirs"]).unwrap();
    point(oids[2]);
    f.commit_staged(21, "theirs").unwrap();
    f.git(&["switch", "main"]).unwrap();
    point(oids[1]);
    f.commit_staged(22, "ours").unwrap();
    assert!(f.git(&["merge", "--no-edit", "theirs"]).is_err());
    f
}

const OID_A: &str = "1111111111111111111111111111111111111111";
const OID_B: &str = "2222222222222222222222222222222222222222";
const OID_C: &str = "3333333333333333333333333333333333333333";

#[test]
fn a_gitlink_conflict_has_all_its_sides_and_is_not_text() {
    let f = special_conflict("160000", "sub", [OID_A, OID_B, OID_C]);
    let repo = open(&f);

    let sides = repo.conflict_sides("sub").unwrap();

    assert!(sides.base.is_some() && sides.ours.is_some() && sides.theirs.is_some());
    assert!(!sides.is_text());
    assert!(repo.resolve_with_text("sub", "x", None).is_err());
}

#[test]
fn taking_a_side_of_a_gitlink_conflict_keeps_the_gitlink_at_that_commit() {
    let f = special_conflict("160000", "sub", [OID_A, OID_B, OID_C]);
    let repo = open(&f);

    repo.resolve_with("sub", git_engine::ConflictSide::Theirs, None)
        .unwrap();

    let staged = f.git(&["ls-files", "-s", "sub"]).unwrap();
    assert_eq!(staged.trim(), format!("160000 {OID_C} 0\tsub"));
}

#[test]
fn a_symlink_conflict_is_taken_whole_and_never_edited_as_text() {
    let f = special_conflict(
        "120000",
        "cfg",
        ["../a/config", "../b/config", "../c/config"],
    );
    let repo = open(&f);

    let sides = repo.conflict_sides("cfg").unwrap();

    assert!(!sides.is_text());
    assert!(repo.resolve_with_text("cfg", "../c/config", None).is_err());
    repo.resolve_with("cfg", git_engine::ConflictSide::Theirs, None)
        .unwrap();
    let staged = f.git(&["ls-files", "-s", "cfg"]).unwrap();
    assert!(staged.starts_with("120000 "), "{staged}");
}

// A solver opened for the first conflict of a path must not write into a later one.
#[test]
fn a_resolution_of_stages_that_are_gone_is_refused() {
    let (f, path) = three_sided();
    let repo = open(&f);
    let old = repo.conflict_sides(&path).unwrap().stages;
    assert!(old.base.is_some() && old.ours.is_some() && old.theirs.is_some());

    f.git(&["merge", "--abort"]).unwrap();
    f.git(&["switch", "theirs"]).unwrap();
    f.write_file("shared.txt", "their other line\nkeep\n")
        .unwrap();
    f.git(&["add", "--", "shared.txt"]).unwrap();
    f.commit_staged(13, "their second change").unwrap();
    f.git(&["switch", "main"]).unwrap();
    let _ = f.git(&["merge", "theirs"]);
    let now = repo.conflict_sides(&path).unwrap().stages;
    assert_ne!(old, now);

    let before = std::fs::read(f.path().join(&path)).unwrap();
    let err = repo
        .resolve_with_text(&path, "merged\n", Some(&old))
        .unwrap_err();
    assert!(err.to_string().contains("changed since"), "{err}");
    assert!(
        repo.resolve_with(&path, git_engine::ConflictSide::Theirs, Some(&old))
            .is_err()
    );
    assert_eq!(std::fs::read(f.path().join(&path)).unwrap(), before);
    assert_eq!(
        repo.conflicted_paths().unwrap(),
        std::slice::from_ref(&path)
    );

    repo.resolve_with_text(&path, "merged\n", Some(&now))
        .unwrap();
    assert!(repo.conflicted_paths().unwrap().is_empty());
}

fn utf16le(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// `*.rc` is UTF-16LE in the working tree and UTF-8 in the repository.
fn utf16_conflict() -> (test_fixtures::Fixture, String) {
    let f = test_fixtures::empty().unwrap();
    f.write_file(".gitattributes", "*.rc working-tree-encoding=UTF-16LE\n")
        .unwrap();
    let commit = |text: &str, index: i64| {
        std::fs::write(f.path().join("a.rc"), utf16le(text)).unwrap();
        f.git(&["add", "--", ".gitattributes", "a.rc"]).unwrap();
        f.commit_staged(index, "side").unwrap();
    };
    commit("line1\nbase\nline3\n", 10);
    f.git(&["switch", "-c", "theirs"]).unwrap();
    commit("line1\ntheirs\nline3\n", 11);
    f.git(&["switch", "main"]).unwrap();
    commit("line1\nours\nline3\n", 12);
    let _ = f.git(&["merge", "theirs"]);
    (f, "a.rc".to_owned())
}

// The sides are the repository's UTF-8 already; a second pass through the clean filter
// read them as UTF-16 and staged noise, or refused after the file was overwritten.
#[test]
fn a_text_resolution_of_an_encoded_file_is_staged_as_utf8_and_written_as_the_file_is() {
    for text in ["line1\nmerged!\nline3\n", "line1\nmerged\nline3\n"] {
        let (f, path) = utf16_conflict();
        let repo = open(&f);

        repo.resolve_with_text(&path, text, None).unwrap();

        assert_eq!(f.git(&["show", ":a.rc"]).unwrap(), text);
        assert_eq!(
            std::fs::read(f.path().join(&path)).unwrap(),
            utf16le(text),
            "{text:?}"
        );
        assert!(repo.conflicted_paths().unwrap().is_empty());
    }
}
