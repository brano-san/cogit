#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Three-way line merge (doc/08-diff-engine.md §8). A region is a conflict only when both
//! sides changed the same lines differently; everything else resolves, and says so.

use diff_engine::{Origin, Region, merge3};

fn text(lines: &[&str]) -> String {
    lines.join("\n")
}

fn kinds(regions: &[Region]) -> Vec<&'static str> {
    regions
        .iter()
        .map(|region| match region {
            Region::Clean {
                origin: Origin::Unchanged,
                ..
            } => "same",
            Region::Clean {
                origin: Origin::Ours,
                ..
            } => "ours",
            Region::Clean {
                origin: Origin::Theirs,
                ..
            } => "theirs",
            Region::Clean {
                origin: Origin::Both,
                ..
            } => "both",
            Region::Clean {
                origin: Origin::Syntactic,
                ..
            } => "syntactic",
            Region::Conflict { .. } => "conflict",
        })
        .collect()
}

#[test]
fn three_identical_files_are_one_unchanged_region() {
    let same = text(&["a", "b", "c"]);
    let merged = merge3(&same, &same, &same);
    assert_eq!(kinds(&merged), ["same"]);
}

#[test]
fn a_change_on_one_side_only_is_taken_without_asking() {
    let base = text(&["a", "b", "c"]);
    let ours = text(&["a", "B", "c"]);
    let merged = merge3(&base, &ours, &base);
    assert_eq!(kinds(&merged), ["same", "ours", "same"]);
}

#[test]
fn a_change_on_the_other_side_only_is_taken_too() {
    let base = text(&["a", "b", "c"]);
    let theirs = text(&["a", "B", "c"]);
    assert_eq!(
        kinds(&merge3(&base, &base, &theirs)),
        ["same", "theirs", "same"]
    );
}

#[test]
fn the_same_edit_on_both_sides_is_not_a_conflict() {
    let base = text(&["a", "b", "c"]);
    let edited = text(&["a", "B", "c"]);
    assert_eq!(
        kinds(&merge3(&base, &edited, &edited)),
        ["same", "both", "same"]
    );
}

#[test]
fn two_different_edits_to_the_same_line_conflict() {
    let base = text(&["a", "b", "c"]);
    let ours = text(&["a", "OURS", "c"]);
    let theirs = text(&["a", "THEIRS", "c"]);
    assert_eq!(
        kinds(&merge3(&base, &ours, &theirs)),
        ["same", "conflict", "same"]
    );
}

#[test]
fn edits_in_different_places_both_land() {
    let base = text(&["a", "b", "c", "d"]);
    let ours = text(&["A", "b", "c", "d"]);
    let theirs = text(&["a", "b", "c", "D"]);
    let merged = merge3(&base, &ours, &theirs);

    assert_eq!(kinds(&merged), ["ours", "same", "theirs"]);
    assert_eq!(resolved(&merged), text(&["A", "b", "c", "D"]));
}

#[test]
fn a_conflict_keeps_all_three_sides_for_the_user_to_pick_from() {
    let base = text(&["a", "b", "c"]);
    let ours = text(&["a", "OURS", "c"]);
    let theirs = text(&["a", "THEIRS", "c"]);
    let merged = merge3(&base, &ours, &theirs);

    match &merged[1] {
        Region::Conflict { base, ours, theirs } => {
            assert_eq!(base, &["b"]);
            assert_eq!(ours, &["OURS"]);
            assert_eq!(theirs, &["THEIRS"]);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_line_added_by_one_side_is_kept() {
    let base = text(&["a", "c"]);
    let ours = text(&["a", "b", "c"]);
    let merged = merge3(&base, &ours, &base);
    assert_eq!(resolved(&merged), ours);
}

#[test]
fn a_line_deleted_by_one_side_stays_deleted() {
    let base = text(&["a", "b", "c"]);
    let theirs = text(&["a", "c"]);
    let merged = merge3(&base, &base, &theirs);
    assert_eq!(resolved(&merged), theirs);
}

#[test]
fn deleting_a_line_one_side_edited_is_a_conflict() {
    let base = text(&["a", "b", "c"]);
    let ours = text(&["a", "B", "c"]);
    let theirs = text(&["a", "c"]);
    assert!(
        merge3(&base, &ours, &theirs)
            .iter()
            .any(Region::is_conflict)
    );
}

#[test]
fn touching_neighbouring_lines_is_not_the_same_as_touching_one() {
    let base = text(&["a", "b", "c", "d", "e"]);
    let ours = text(&["a", "B", "c", "d", "e"]);
    let theirs = text(&["a", "b", "c", "D", "e"]);
    assert!(
        !merge3(&base, &ours, &theirs)
            .iter()
            .any(Region::is_conflict)
    );
}

#[test]
fn adjoining_edits_conflict_because_their_order_is_not_ours_to_guess() {
    let base = text(&["a", "b", "c"]);
    let ours = text(&["a", "B", "c"]);
    let theirs = text(&["a", "b", "B2", "c"]);
    // Ours replaces line 2; theirs inserts right after it. Which comes first is a
    // judgement, and a merge tool that guesses silently is the dangerous kind.
    assert!(
        merge3(&base, &ours, &theirs)
            .iter()
            .any(Region::is_conflict)
    );
}

#[test]
fn an_empty_base_with_two_new_files_conflicts() {
    let merged = merge3("", "ours\n", "theirs\n");
    assert!(merged.iter().any(Region::is_conflict));
}

#[test]
fn crlf_does_not_turn_every_line_into_a_conflict() {
    let base = "a\r\nb\r\nc\r\n";
    let ours = "a\nB\nc\n";
    assert_eq!(kinds(&merge3(base, ours, base)), ["same", "ours", "same"]);
}

#[test]
fn counting_conflicts_is_what_the_navigation_bar_needs() {
    let base = text(&["a", "b", "c", "d", "e"]);
    let ours = text(&["A", "b", "C", "d", "e"]);
    let theirs = text(&["A2", "b", "C2", "d", "e"]);
    assert_eq!(
        merge3(&base, &ours, &theirs)
            .iter()
            .filter(|region| region.is_conflict())
            .count(),
        2
    );
}

fn resolved(regions: &[Region]) -> String {
    regions
        .iter()
        .flat_map(|region| match region {
            Region::Clean { lines, .. } => lines.clone(),
            Region::Conflict { ours, .. } => ours.clone(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}
