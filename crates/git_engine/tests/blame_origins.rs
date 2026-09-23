// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Blame for the Investigate window: `git blame --porcelain -M -C -C`, so every line names
//! the commit, the file and the line it came from, even across moves between files.

use git_engine::{BlameReport, LineChange, OriginLine, RepoHandle};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn commit_of<'a>(report: &'a BlameReport, line: &OriginLine) -> &'a git_engine::BlameCommit {
    let source = &report.sources[line.source as usize];
    &report.commits[source.commit as usize]
}

fn source_path<'a>(report: &'a BlameReport, line: &OriginLine) -> &'a str {
    &report.sources[line.source as usize].path
}

fn stage(f: &test_fixtures::Fixture, name: &str, contents: &str) {
    f.write_file(name, contents).unwrap();
    f.git(&["add", "--", name]).unwrap();
}

#[test]
fn every_line_names_its_commit_text_and_number_in_file_order() {
    let f = test_fixtures::linear(1).unwrap();
    stage(&f, "story.txt", "one\ntwo\n");
    let first = f.commit_staged(10, "first").unwrap();
    stage(&f, "story.txt", "one\ntwo\nthree\n");
    let second = f.commit_staged(11, "second").unwrap();

    let report = open(&f).blame_origins("story.txt", None, false).unwrap();

    let lines: Vec<(u32, &str, &str)> = report
        .lines
        .iter()
        .map(|line| {
            (
                line.line,
                line.text.as_str(),
                commit_of(&report, line).oid.as_str(),
            )
        })
        .collect();
    assert_eq!(
        lines,
        [
            (1, "one", first.as_str()),
            (2, "two", first.as_str()),
            (3, "three", second.as_str())
        ]
    );
    assert_eq!(commit_of(&report, &report.lines[2]).summary, "second");
    assert_eq!(
        commit_of(&report, &report.lines[2]).author,
        test_fixtures::AUTHOR_NAME
    );
}

#[test]
fn a_block_moved_from_another_file_keeps_its_original_commit_file_and_line() {
    let f = test_fixtures::linear(1).unwrap();
    let moved = "fn a_long_function_name_that_survives_the_move_between_files() {\n    \
                 let computed_value_for_the_detector = add_many_numbers(1, 2, 3, 4);\n}\n";
    stage(&f, "donor.txt", &format!("keep this line\n{moved}"));
    let written = f.commit_staged(10, "write the donor").unwrap();

    stage(&f, "donor.txt", "keep this line\n");
    stage(&f, "story.txt", &format!("header of the story\n{moved}"));
    f.commit_staged(11, "move the function").unwrap();

    let report = open(&f)
        .blame_origins("story.txt", Some("HEAD"), false)
        .unwrap();

    let second = &report.lines[1];
    assert_eq!(commit_of(&report, second).oid, written);
    assert_eq!(source_path(&report, second), "donor.txt");
    assert_eq!(second.orig_line, 2);
}

#[test]
fn a_replaced_line_is_modified_and_an_appended_one_is_added() {
    let f = test_fixtures::linear(1).unwrap();
    stage(&f, "story.txt", "one\ntwo\nthree\n");
    f.commit_staged(10, "write").unwrap();
    stage(&f, "story.txt", "one\nTWO\nthree\nfour\n");
    f.commit_staged(11, "edit").unwrap();

    let report = open(&f).blame_origins("story.txt", None, false).unwrap();

    assert_eq!(report.lines[1].change, LineChange::Modified);
    assert_eq!(report.lines[3].change, LineChange::Added);
}

#[test]
fn lines_of_the_first_commit_are_added_and_it_is_a_boundary() {
    let f = test_fixtures::linear(1).unwrap();

    let report = open(&f).blame_origins("file0.txt", None, false).unwrap();

    assert_eq!(report.lines[0].change, LineChange::Added);
    assert!(commit_of(&report, &report.lines[0]).boundary);
}

#[test]
fn a_line_written_while_resolving_a_merge_is_marked_as_coming_from_a_merge() {
    let f = test_fixtures::linear(1).unwrap();
    stage(&f, "story.txt", "a\nb\nc\n");
    f.commit_staged(10, "base").unwrap();
    f.git(&["checkout", "-q", "-b", "side"]).unwrap();
    stage(&f, "story.txt", "A\nb\nc\n");
    f.commit_staged(11, "side edit").unwrap();
    f.git(&["checkout", "-q", "main"]).unwrap();
    stage(&f, "story.txt", "a\nb\nC\n");
    f.commit_staged(12, "main edit").unwrap();
    f.git(&["merge", "--no-ff", "--no-commit", "side"]).unwrap();
    stage(&f, "story.txt", "A\nB from the merge\nC\n");
    let merge = f.commit_staged(13, "merge side").unwrap();

    let report = open(&f).blame_origins("story.txt", None, false).unwrap();

    let line = &report.lines[1];
    assert_eq!(commit_of(&report, line).oid, merge);
    assert!(commit_of(&report, line).merge);
    assert_eq!(line.change, LineChange::Modified);
    assert!(!commit_of(&report, &report.lines[0]).merge);
}

#[test]
fn the_working_tree_is_blamed_when_no_revision_is_given() {
    let f = test_fixtures::linear(1).unwrap();
    stage(&f, "story.txt", "one\ntwo\n");
    let head = f.commit_staged(10, "write").unwrap();
    f.write_file("story.txt", "one\nTWO\nthree\n").unwrap();

    let report = open(&f).blame_origins("story.txt", None, false).unwrap();

    let edited = commit_of(&report, &report.lines[1]);
    assert!(edited.uncommitted);
    assert_eq!(report.lines[1].change, LineChange::Modified);
    assert_eq!(report.lines[2].change, LineChange::Added);
    assert_eq!(commit_of(&report, &report.lines[0]).oid, head);
    assert!(!commit_of(&report, &report.lines[0]).uncommitted);
}

#[test]
fn a_revision_blames_the_file_as_it_was_then() {
    let f = test_fixtures::linear(1).unwrap();
    stage(&f, "story.txt", "one\n");
    let first = f.commit_staged(10, "write").unwrap();
    stage(&f, "story.txt", "one\ntwo\n");
    f.commit_staged(11, "extend").unwrap();

    let report = open(&f)
        .blame_origins("story.txt", Some(&first), false)
        .unwrap();

    assert_eq!(report.lines.len(), 1);
}

#[test]
fn a_line_edited_in_the_commit_that_renamed_the_file_points_back_to_the_old_name() {
    let f = test_fixtures::linear(1).unwrap();
    let body: String = (1..=12).map(|n| format!("line number {n}\n")).collect();
    stage(&f, "story.txt", &body);
    let written = f.commit_staged(10, "write").unwrap();
    f.git(&["mv", "story.txt", "tale.txt"]).unwrap();
    stage(
        &f,
        "tale.txt",
        &body.replace("line number 5\n", "line number five\n"),
    );
    f.commit_staged(11, "rename and edit").unwrap();

    let report = open(&f).blame_origins("tale.txt", None, false).unwrap();

    let edited = &report.sources[report.lines[4].source as usize];
    let previous = edited
        .previous
        .as_ref()
        .expect("the edit has a parent version");
    assert_eq!(previous.oid, written);
    assert_eq!(previous.path, "story.txt");
    assert_eq!(report.lines[4].change, LineChange::Modified);
    assert_eq!(source_path(&report, &report.lines[0]), "story.txt");
}

#[test]
fn ignoring_whitespace_looks_past_a_reindentation() {
    let f = test_fixtures::linear(1).unwrap();
    stage(&f, "story.txt", "one\ntwo\n");
    let written = f.commit_staged(10, "write").unwrap();
    stage(&f, "story.txt", "one\n    two\n");
    let indented = f.commit_staged(11, "indent").unwrap();

    let plain = open(&f).blame_origins("story.txt", None, false).unwrap();
    let ignoring = open(&f).blame_origins("story.txt", None, true).unwrap();

    assert_eq!(commit_of(&plain, &plain.lines[1]).oid, indented);
    assert_eq!(commit_of(&ignoring, &ignoring.lines[1]).oid, written);
}

#[test]
fn each_commit_appears_once_in_the_table() {
    let f = test_fixtures::linear(1).unwrap();
    stage(&f, "story.txt", "one\ntwo\nthree\n");
    f.commit_staged(10, "write").unwrap();
    stage(&f, "story.txt", "one\nTWO\nthree\n");
    f.commit_staged(11, "edit").unwrap();

    let report = open(&f).blame_origins("story.txt", None, false).unwrap();

    assert_eq!(report.commits.len(), 2);
    assert_eq!(report.lines[0].source, report.lines[2].source);
}

#[test]
fn a_long_file_is_blamed_to_its_last_line() {
    let f = test_fixtures::linear(1).unwrap();
    let body: String = (1..=12_000).map(|n| format!("line {n}\n")).collect();
    stage(&f, "long.txt", &body);
    f.commit_staged(10, "long").unwrap();

    let report = open(&f).blame_origins("long.txt", None, false).unwrap();

    assert_eq!(report.lines.len(), 12_000);
    assert_eq!(report.lines[11_999].text, "line 12000");
}

#[test]
fn a_carriage_return_is_not_part_of_the_text() {
    let f = test_fixtures::linear(1).unwrap();
    stage(&f, "dos.txt", "one\r\ntwo\r\n");
    f.commit_staged(10, "dos").unwrap();

    let report = open(&f).blame_origins("dos.txt", None, false).unwrap();

    assert_eq!(report.lines[0].text, "one");
    assert_eq!(report.lines[1].text, "two");
}

#[test]
fn a_path_that_does_not_exist_is_a_typed_error() {
    let f = test_fixtures::linear(1).unwrap();

    assert!(open(&f).blame_origins("nowhere.txt", None, false).is_err());
}
