// Integration tests are test code by definition, but `allow-unwrap-in-tests` in
// clippy.toml only covers the bodies of `#[test]` functions — helpers beside them are
// still linted. Panicking is how a test reports failure, so allow it for the file.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Fixtures whose point is the *content* of files rather than the shape of history,
//! plus the environment isolation everything else depends on.

use test_fixtures::{crlf_files, empty, filemode_change, renames, unicode_paths, with_stashes};

#[test]
fn the_developer_global_config_is_invisible_to_fixtures() {
    // The single most valuable test in this crate. Without isolation, a developer with
    // `core.autocrlf=true` gets different fixtures than one without, and "green on my
    // machine, red on yours" becomes unfixable.
    let f = empty().unwrap();
    // An error here means there is no global scope at all, which is equally desirable.
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
    // Mixed endings are a real-world source of phantom diffs, so the fixture has one.
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
    // Git on Windows does not track the executable bit in the working tree, so the
    // fixture has to set it in the index. The tree is where the truth lives.
    let f = filemode_change().unwrap();
    let entry = f.git(&["ls-tree", "HEAD", "--", "script.sh"]).unwrap();
    assert!(
        entry.starts_with("100755"),
        "expected mode 100755, got: {entry}"
    );
}

#[test]
fn filemode_change_keeps_the_content_identical() {
    // Only the mode changed; a diff engine must show that and nothing else.
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
