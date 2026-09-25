// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use diff_engine::{
    DiffOptions, FileDiff, PatchError, PatchRequest, PatchShape, PatchSides, build_patch, diff_text,
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
    }
}

/// The patch for `selected` lines of the diff from `old` to `new`, cut from those files.
fn patch(old: &str, new: &str, selected: &[(u32, bool)], shape: PatchShape) -> String {
    build_patch(&request(old, new, selected), shape, sides(old, new)).unwrap()
}

fn sides<'a>(old: &'a str, new: &'a str) -> PatchSides<'a> {
    PatchSides {
        old: old.as_bytes(),
        new: new.as_bytes(),
    }
}

#[test]
fn a_patch_starts_with_the_file_headers_git_apply_expects() {
    let patch = patch("a\n", "b\n", &[(1, true), (1, false)], FORWARD);

    assert!(
        patch.starts_with("--- a/file.txt\n+++ b/file.txt\n@@"),
        "{patch}"
    );
}

#[test]
fn selecting_one_insert_leaves_the_others_out() {
    let old = "keep\n";
    let new = "keep\nfirst\nsecond\n";

    let patch = patch(old, new, &[(2, false)], FORWARD);

    assert!(patch.contains("+first"), "{patch}");
    assert!(!patch.contains("+second"), "{patch}");
}

#[test]
fn an_unselected_delete_becomes_context_rather_than_disappearing() {
    let old = "one\ntwo\n";
    let new = "";

    let patch = patch(old, new, &[(1, true)], FORWARD);

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

    let patch = patch(old, new, &[(1, true)], FORWARD);

    assert!(patch.contains("@@ -1,2 +1,1 @@"), "{patch}");
}

#[test]
fn staging_only_deletions_while_insertions_exist_in_the_same_hunk() {
    let old = "alpha\nbeta\n";
    let new = "alpha\ngamma\n";

    let patch = patch(old, new, &[(2, true)], FORWARD);

    assert!(patch.contains("-beta"), "{patch}");
    assert!(!patch.contains("+gamma"), "{patch}");
}

#[test]
fn selecting_nothing_produces_no_patch_at_all() {
    assert_eq!(
        build_patch(&request("a\n", "b\n", &[]), FORWARD, sides("a\n", "b\n")),
        Err(PatchError::NothingSelected)
    );
}

#[test]
fn crlf_is_restored_so_git_apply_does_not_rewrite_the_file() {
    let patch = patch(
        "a\r\nb\r\n",
        "a\r\nB\r\n",
        &[(2, true), (2, false)],
        FORWARD,
    );

    assert!(patch.contains("-b\r\n"), "INV-08: {patch:?}");
    assert!(patch.contains("+B\r\n"), "INV-08: {patch:?}");
    assert!(
        patch.starts_with("--- a/file.txt\n"),
        "the patch envelope itself stays LF: {patch:?}"
    );
}

#[test]
fn a_missing_final_newline_is_marked() {
    let patch = patch("a\nb", "a\nB", &[(2, true), (2, false)], FORWARD);

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
        sides("", "fresh\n"),
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
        sides("gone\n", ""),
    )
    .unwrap();

    assert!(patch.contains("+++ /dev/null"), "{patch}");
}

#[test]
fn every_line_of_the_patch_ends_with_a_newline() {
    let patch = patch("a\nb\nc\n", "a\nB\nc\n", &[(2, true), (2, false)], FORWARD);

    assert!(patch.ends_with('\n'), "git apply refuses a truncated patch");
}

#[test]
fn each_line_keeps_the_ending_it_has_on_its_own_side() {
    let old = "a\r\nb\nc\r\n";
    let new = "a\r\nB\r\nc\r\n";

    let patch = patch(old, new, &[(2, true), (2, false)], FORWARD);

    assert!(patch.contains(" a\r\n-b\n+B\r\n c\r\n"), "{patch:?}");
}

#[test]
fn a_lone_cr_is_refused_rather_than_misnumbered() {
    let (old, new) = ("a\rb\n", "a\rB\n");

    let refused = build_patch(&request(old, new, &[(2, true)]), FORWARD, sides(old, new));

    assert_eq!(refused, Err(PatchError::BareCarriageReturn));
}
