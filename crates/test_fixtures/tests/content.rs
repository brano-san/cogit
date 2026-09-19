// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use test_fixtures::{crlf_files, empty, filemode_change, renames, unicode_paths, with_stashes};

#[test]
fn the_developer_global_config_is_invisible_to_fixtures() {
    // Without isolation a developer with `core.autocrlf=true` gets different fixtures.
    let f = empty().unwrap();
    if let Ok(listing) = f.git(&["config", "--global", "--list"]) {
        assert!(
            listing.trim().is_empty(),
            "leaked global config:\n{listing}"
        );
    }
}

#[test]
fn fixtures_pin_autocrlf_off() {
    let f = empty().unwrap();
    assert_eq!(f.git(&["config", "core.autocrlf"]).unwrap().trim(), "false");
}

#[test]
fn crlf_file_keeps_carriage_returns_in_the_blob() {
    let f = crlf_files().unwrap();
    let blob = f.git(&["cat-file", "-p", "HEAD:crlf.txt"]).unwrap();
    assert!(
        blob.contains("\r\n"),
        "CRLF must survive into the object database"
    );
}

#[test]
fn lf_file_has_no_carriage_returns() {
    let f = crlf_files().unwrap();
    let blob = f.git(&["cat-file", "-p", "HEAD:lf.txt"]).unwrap();
    assert!(!blob.contains('\r'), "LF file must stay clean");
}

#[test]
fn mixed_file_contains_both_endings() {
    let f = crlf_files().unwrap();
    let blob = f.git(&["cat-file", "-p", "HEAD:mixed.txt"]).unwrap();
    assert!(blob.contains("\r\n"), "expected a CRLF line");
    assert!(blob.lines().count() > 1);
    assert!(
        blob.replace("\r\n", "").contains('\n'),
        "expected a bare LF line too"
    );
}

#[test]
fn rename_is_reported_as_a_rename() {
    let f = renames().unwrap();
    let status = f
        .git(&["diff", "-M", "--name-status", "HEAD~1", "HEAD"])
        .unwrap();
    assert!(status.starts_with('R'), "expected a rename, got: {status}");
}

#[test]
fn filemode_change_sets_the_executable_bit() {
    let f = filemode_change().unwrap();
    let entry = f.git(&["ls-tree", "HEAD", "--", "script.sh"]).unwrap();
    assert!(
        entry.starts_with("100755"),
        "expected mode 100755, got: {entry}"
    );
}

#[test]
fn filemode_change_keeps_the_content_identical() {
    let f = filemode_change().unwrap();
    let before = f.git(&["cat-file", "-p", "HEAD~1:script.sh"]).unwrap();
    let after = f.git(&["cat-file", "-p", "HEAD:script.sh"]).unwrap();
    assert_eq!(before, after);
}

#[test]
fn unicode_paths_are_tracked_verbatim() {
    let f = unicode_paths().unwrap();
    let files = f.git(&["ls-files"]).unwrap();
    assert!(
        files.contains("файл.txt"),
        "cyrillic path missing from:\n{files}"
    );
    assert!(
        files.contains("with space.txt"),
        "spaced path missing from:\n{files}"
    );
}

#[test]
fn stash_count_matches_the_request() {
    let f = with_stashes(3).unwrap();
    let list = f.git(&["stash", "list"]).unwrap();
    assert_eq!(list.trim().lines().count(), 3);
}

#[test]
fn stashing_leaves_the_working_tree_clean() {
    let f = with_stashes(2).unwrap();
    let status = f.git(&["status", "--porcelain"]).unwrap();
    assert!(
        status.trim().is_empty(),
        "expected a clean tree, got:\n{status}"
    );
}
