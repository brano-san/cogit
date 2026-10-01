#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The Conflict Solver aligns its panes by stretch: every stretch carries what the base,
//! our side, their side and the merge have there (doc/08-diff-engine.md §8, §13).

use diff_engine::{SolverKind, SolverRegion, merge3_sides};

fn text(lines: &[&str]) -> String {
    format!("{}\n", lines.join("\n"))
}

fn kinds(regions: &[SolverRegion]) -> Vec<SolverKind> {
    regions.iter().map(|region| region.kind).collect()
}

fn strings(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|line| (*line).to_owned()).collect()
}

#[test]
fn identical_sides_are_one_equal_stretch_with_the_lines_everywhere() {
    let same = text(&["a", "b"]);
    let regions = merge3_sides(&same, &same, &same, None);
    assert_eq!(kinds(&regions), [SolverKind::Equal]);
    assert_eq!(regions[0].ours, strings(&["a", "b"]));
    assert_eq!(regions[0].theirs, strings(&["a", "b"]));
    assert_eq!(regions[0].base, strings(&["a", "b"]));
    assert_eq!(regions[0].result, strings(&["a", "b"]));
}

#[test]
fn a_change_of_ours_alone_keeps_what_theirs_has_which_is_the_base() {
    let base = text(&["a", "b", "c"]);
    let ours = text(&["a", "B", "c"]);
    let regions = merge3_sides(&base, &ours, &base, None);
    assert_eq!(
        kinds(&regions),
        [SolverKind::Equal, SolverKind::Ours, SolverKind::Equal]
    );
    let changed = &regions[1];
    assert_eq!(changed.base, strings(&["b"]));
    assert_eq!(changed.ours, strings(&["B"]));
    assert_eq!(changed.theirs, strings(&["b"]));
    assert_eq!(changed.result, strings(&["B"]));
}

#[test]
fn a_change_of_theirs_alone_is_theirs() {
    let base = text(&["a", "b"]);
    let theirs = text(&["a", "b", "c"]);
    let regions = merge3_sides(&base, &base, &theirs, None);
    assert_eq!(kinds(&regions), [SolverKind::Equal, SolverKind::Theirs]);
    assert_eq!(regions[1].base, Vec::<String>::new());
    assert_eq!(regions[1].theirs, strings(&["c"]));
    assert_eq!(regions[1].result, strings(&["c"]));
}

#[test]
fn the_same_edit_on_both_sides_is_both() {
    let base = text(&["a", "b"]);
    let edited = text(&["a", "x"]);
    assert_eq!(
        kinds(&merge3_sides(&base, &edited, &edited, None)),
        [SolverKind::Equal, SolverKind::Both]
    );
}

#[test]
fn a_conflict_holds_all_three_versions_and_the_base_as_its_undecided_result() {
    let base = text(&["a", "b", "c"]);
    let ours = text(&["a", "ours", "c"]);
    let theirs = text(&["a", "theirs", "c"]);
    let regions = merge3_sides(&base, &ours, &theirs, None);
    assert_eq!(
        kinds(&regions),
        [SolverKind::Equal, SolverKind::Conflict, SolverKind::Equal]
    );
    let conflict = &regions[1];
    assert_eq!(conflict.base, strings(&["b"]));
    assert_eq!(conflict.ours, strings(&["ours"]));
    assert_eq!(conflict.theirs, strings(&["theirs"]));
    assert_eq!(conflict.result, strings(&["b"]));
}

#[test]
fn add_add_without_a_base_is_one_conflict_over_two_whole_files() {
    let regions = merge3_sides("", "one\ntwo\n", "uno\ndos\n", None);
    assert_eq!(kinds(&regions), [SolverKind::Conflict]);
    assert_eq!(regions[0].base, Vec::<String>::new());
    assert_eq!(regions[0].ours, strings(&["one", "two"]));
    assert_eq!(regions[0].theirs, strings(&["uno", "dos"]));
}

#[test]
fn crlf_sides_merge_like_lf_ones() {
    let base = "a\r\nb\r\n";
    let ours = "a\r\nB\r\n";
    let regions = merge3_sides(base, ours, base, None);
    assert_eq!(kinds(&regions), [SolverKind::Equal, SolverKind::Ours]);
    assert_eq!(regions[1].ours, strings(&["B"]));
}

#[test]
fn every_stretch_of_the_sides_rebuilds_each_side() {
    let base = text(&["1", "2", "3", "4", "5", "6", "7"]);
    let ours = text(&["1", "2", "three", "4", "5", "6", "7", "8"]);
    let theirs = text(&["one", "2", "3", "4", "five", "6", "7"]);
    let regions = merge3_sides(&base, &ours, &theirs, None);
    let join = |pick: fn(&SolverRegion) -> &Vec<String>| {
        regions
            .iter()
            .flat_map(|region| pick(region).clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        join(|r| &r.base),
        strings(&["1", "2", "3", "4", "5", "6", "7"])
    );
    assert_eq!(
        join(|r| &r.ours),
        strings(&["1", "2", "three", "4", "5", "6", "7", "8"])
    );
    assert_eq!(
        join(|r| &r.theirs),
        strings(&["one", "2", "3", "4", "five", "6", "7"])
    );
    assert_eq!(
        join(|r| &r.result),
        strings(&["one", "2", "three", "4", "five", "6", "7", "8"])
    );
}

#[test]
fn nothing_at_all_is_one_empty_equal_stretch() {
    let regions = merge3_sides("", "", "", None);
    assert_eq!(kinds(&regions), [SolverKind::Equal]);
}
