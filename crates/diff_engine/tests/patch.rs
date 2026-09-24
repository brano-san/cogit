// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use diff_engine::{
    DiffOptions, FileDiff, LineEnding, PatchRequest, PatchShape, build_patch, diff_text,
};

/// Stage: forward, onto a file both sides of the diff have.
const FORWARD: PatchShape = PatchShape {
    reverse: false,
    old_exists: true,
    new_exists: true,
};

fn hunks(old: &str, new: &str) -> Vec<diff_engine::Hunk> {
    match diff_text(old, new, &DiffOptions::default()) {
        FileDiff::Text { hunks, .. } => hunks,
        other => panic!("expected a text diff, got {other:?}"),
    }
}

fn request(old: &str, new: &str, selected: &[(u32, bool)]) -> PatchRequest {
    PatchRequest {
        path: "file.txt".to_owned(),
        hunks: hunks(old, new),
        selected_deletes: selected
            .iter()
            .filter(|(_, is_delete)| *is_delete)
            .map(|(line, _)| *line)
            .collect(),
        selected_inserts: selected
            .iter()
            .filter(|(_, is_delete)| !*is_delete)
            .map(|(line, _)| *line)
            .collect(),
        line_ending: LineEnding::Lf,
    }
}

#[test]
fn a_patch_starts_with_the_file_headers_git_apply_expects() {
    let patch = build_patch(&request("a\n", "b\n", &[(1, true), (1, false)]), FORWARD).unwrap();

    assert!(
        patch.starts_with("--- a/file.txt\n+++ b/file.txt\n@@"),
        "{patch}"
    );
}

#[test]
fn selecting_one_insert_leaves_the_others_out() {
    let old = "keep\n";
    let new = "keep\nfirst\nsecond\n";

    let patch = build_patch(&request(old, new, &[(2, false)]), FORWARD).unwrap();

    assert!(patch.contains("+first"), "{patch}");
    assert!(!patch.contains("+second"), "{patch}");
}

#[test]
fn an_unselected_delete_becomes_context_rather_than_disappearing() {
    let old = "one\ntwo\n";
    let new = "";

    let patch = build_patch(&request(old, new, &[(1, true)]), FORWARD).unwrap();

    assert!(patch.contains("-one"), "{patch}");
    assert!(
        patch.contains(" two"),
        "an unselected delete stays as context: {patch}"
    );
}

#[test]
fn the_hunk_header_counts_only_the_lines_the_patch_carries() {
    let old = "one\ntwo\n";
    let new = "";

    let patch = build_patch(&request(old, new, &[(1, true)]), FORWARD).unwrap();

    assert!(patch.contains("@@ -1,2 +1,1 @@"), "{patch}");
}

#[test]
fn staging_only_deletions_while_insertions_exist_in_the_same_hunk() {
    let old = "alpha\nbeta\n";
    let new = "alpha\ngamma\n";

    let patch = build_patch(&request(old, new, &[(2, true)]), FORWARD).unwrap();

    assert!(patch.contains("-beta"), "{patch}");
    assert!(!patch.contains("+gamma"), "{patch}");
}

#[test]
fn selecting_nothing_produces_no_patch_at_all() {
    assert!(build_patch(&request("a\n", "b\n", &[]), FORWARD).is_none());
}

#[test]
fn crlf_is_restored_so_git_apply_does_not_rewrite_the_file() {
    let mut req = request("a\nb\n", "a\nB\n", &[(2, true), (2, false)]);
    req.line_ending = LineEnding::Crlf;

    let patch = build_patch(&req, FORWARD).unwrap();

    assert!(patch.contains("-b\r\n"), "INV-08: {patch:?}");
    assert!(patch.contains("+B\r\n"), "INV-08: {patch:?}");
    assert!(
        patch.starts_with("--- a/file.txt\n"),
        "the patch envelope itself stays LF: {patch:?}"
    );
}

#[test]
fn a_missing_final_newline_is_marked() {
    let req = request("a\nb", "a\nB", &[(2, true), (2, false)]);

    let patch = build_patch(&req, FORWARD).unwrap();

    assert!(
        patch.contains("\\ No newline at end of file"),
        "without the marker git apply appends one: {patch}"
    );
}

#[test]
fn a_brand_new_file_patches_against_dev_null() {
    let mut req = request("", "fresh\n", &[(1, false)]);
    req.path = "new.txt".to_owned();

    let patch = build_patch(
        &req,
        PatchShape {
            old_exists: false,
            ..FORWARD
        },
    )
    .unwrap();

    assert!(
        patch.starts_with("--- /dev/null\n+++ b/new.txt\n"),
        "{patch}"
    );
}

#[test]
fn a_deleted_file_patches_to_dev_null() {
    let mut req = request("gone\n", "", &[(1, true)]);
    req.path = "old.txt".to_owned();

    let patch = build_patch(
        &req,
        PatchShape {
            new_exists: false,
            ..FORWARD
        },
    )
    .unwrap();

    assert!(patch.contains("+++ /dev/null"), "{patch}");
}

#[test]
fn every_line_of_the_patch_ends_with_a_newline() {
    let patch = build_patch(
        &request("a\nb\nc\n", "a\nB\nc\n", &[(2, true), (2, false)]),
        FORWARD,
    )
    .unwrap();

    assert!(patch.ends_with('\n'), "git apply refuses a truncated patch");
}
