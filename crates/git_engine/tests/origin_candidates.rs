// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Origin candidates: where a block of lines was before the commit that introduced it —
//! in place, elsewhere in the file, or in another file — found by fuzzy matching, so a
//! block that was moved and edited at once is still traced.

use git_engine::{
    Likelihood, LineMatch, OriginCandidate, OriginKind, OriginQuery, OriginReport, PreviousFile,
    RepoHandle,
};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn stage(f: &test_fixtures::Fixture, name: &str, contents: &str) {
    f.write_file(name, contents).unwrap();
    f.git(&["add", "--", name]).unwrap();
}

const BLOCK: &str = "local function rotate(cat, angle)\n\
                     local radians = math.rad(angle)\n\
                     cat.x = cat.x * math.cos(radians)\n\
                     cat.y = cat.y * math.sin(radians)\n\
                     return cat\n\
                     end\n";

/// The block as it lands in its new home: indented, one line edited, so `git blame -C`
/// no longer sees it as the same text.
fn edited_block() -> String {
    BLOCK
        .replace("math.sin(radians)", "math.sin(radians) + offset")
        .lines()
        .map(|line| format!("    {line}\n"))
        .collect()
}

fn query(
    commit: &str,
    path: &str,
    from: u32,
    to: u32,
    line: u32,
    previous: Option<(&str, &str)>,
) -> OriginQuery {
    OriginQuery {
        commit: commit.to_owned(),
        path: path.to_owned(),
        from,
        to,
        line,
        previous: previous.map(|(oid, path)| PreviousFile {
            oid: oid.to_owned(),
            path: path.to_owned(),
        }),
    }
}

fn search(f: &test_fixtures::Fixture, query: &OriginQuery) -> OriginReport {
    open(f)
        .origin_candidates(query, &|| false)
        .unwrap()
        .unwrap()
}

fn best(report: &OriginReport) -> &OriginCandidate {
    &report.candidates[report.best as usize]
}

struct Moved {
    fixture: test_fixtures::Fixture,
    before: String,
    after: String,
}

fn moved_between_files(keep_in_donor: bool) -> Moved {
    let f = test_fixtures::linear(1).unwrap();
    stage(
        &f,
        "donor.lua",
        &format!("print('donor header')\n{BLOCK}print('donor footer')\n"),
    );
    stage(
        &f,
        "story.lua",
        "print('story header')\nprint('story footer')\n",
    );
    let before = f.commit_staged(10, "write the donor").unwrap();

    if !keep_in_donor {
        stage(
            &f,
            "donor.lua",
            "print('donor header')\nprint('donor footer')\n",
        );
    }
    stage(
        &f,
        "story.lua",
        &format!(
            "print('story header')\n{}print('story footer')\n",
            edited_block()
        ),
    );
    let after = f.commit_staged(11, "bring rotate over").unwrap();
    Moved {
        fixture: f,
        before,
        after,
    }
}

#[test]
fn a_block_moved_and_edited_between_files_is_traced_to_the_file_it_left() {
    let m = moved_between_files(false);

    let report = search(
        &m.fixture,
        &query(
            &m.after,
            "story.lua",
            2,
            7,
            4,
            Some((&m.before, "story.lua")),
        ),
    );

    let found = best(&report);
    assert_eq!(found.kind, OriginKind::Moved);
    assert_eq!(found.path, "donor.lua");
    assert_eq!(found.rev, m.before);
    assert_eq!((found.from, found.to), (2, 7));
    assert_eq!(found.likelihood, Likelihood::High);
}

#[test]
fn going_deeper_lands_on_the_same_line_in_the_source() {
    let m = moved_between_files(false);

    let report = search(
        &m.fixture,
        &query(
            &m.after,
            "story.lua",
            2,
            7,
            5,
            Some((&m.before, "story.lua")),
        ),
    );

    let deeper = best(&report).deeper.clone().expect("a parent to go to");
    assert_eq!(deeper.rev, m.before);
    assert_eq!(deeper.path, "donor.lua");
    assert_eq!(deeper.line, 5);
}

#[test]
fn a_block_whose_source_stayed_where_it_was_is_a_copy() {
    let m = moved_between_files(true);

    let report = search(
        &m.fixture,
        &query(
            &m.after,
            "story.lua",
            2,
            7,
            4,
            Some((&m.before, "story.lua")),
        ),
    );

    assert_eq!(best(&report).kind, OriginKind::Copied);
    assert_eq!(best(&report).path, "donor.lua");
}

#[test]
fn a_block_moved_within_the_file_is_found_at_its_old_position() {
    let f = test_fixtures::linear(1).unwrap();
    let filler: String = (1..=8)
        .map(|n| format!("print('unrelated statement number {n}')\n"))
        .collect();
    stage(&f, "story.lua", &format!("{BLOCK}{filler}"));
    let before = f.commit_staged(10, "write").unwrap();
    stage(&f, "story.lua", &format!("{filler}{}", edited_block()));
    let after = f.commit_staged(11, "move rotate down").unwrap();

    let report = search(
        &f,
        &query(&after, "story.lua", 9, 14, 10, Some((&before, "story.lua"))),
    );

    let found = best(&report);
    assert_eq!(found.kind, OriginKind::Moved);
    assert_eq!(found.path, "story.lua");
    assert_eq!((found.from, found.to), (1, 6));
}

#[test]
fn brand_new_lines_appeared_here_as_the_single_likely_origin() {
    let f = test_fixtures::linear(1).unwrap();
    stage(&f, "story.lua", "print('one')\nprint('two')\n");
    let before = f.commit_staged(10, "write").unwrap();
    stage(
        &f,
        "story.lua",
        "print('one')\nlocal velocity = distance / elapsed_time\nprint('two')\n",
    );
    let after = f.commit_staged(11, "add velocity").unwrap();

    let report = search(
        &f,
        &query(&after, "story.lua", 2, 2, 2, Some((&before, "story.lua"))),
    );

    assert_eq!(report.candidates.len(), 1);
    let found = best(&report);
    assert_eq!(found.kind, OriginKind::Appeared);
    assert_eq!(found.likelihood, Likelihood::High);
    let deeper = found
        .deeper
        .clone()
        .expect("the parent version still exists");
    assert_eq!(deeper.rev, before);
    assert_eq!(deeper.path, "story.lua");
}

#[test]
fn a_line_rewritten_in_place_points_at_the_line_it_replaced_first() {
    let f = test_fixtures::linear(1).unwrap();
    stage(
        &f,
        "story.lua",
        "print('one')\nlocal speed = 10 * factor\nprint('three')\n",
    );
    let before = f.commit_staged(10, "write").unwrap();
    stage(
        &f,
        "story.lua",
        "print('one')\nlocal speed = 12 * factor + bonus\nprint('three')\n",
    );
    let after = f.commit_staged(11, "tune").unwrap();

    let report = search(
        &f,
        &query(&after, "story.lua", 2, 2, 2, Some((&before, "story.lua"))),
    );

    let first = &report.candidates[0];
    assert_eq!(first.kind, OriginKind::Modified);
    assert_eq!((first.from, first.to), (2, 2));
    assert_eq!(first.deeper.as_ref().map(|d| d.line), Some(2));
    assert_eq!(best(&report).kind, OriginKind::Modified);
    assert_eq!(
        first.likelihood,
        Likelihood::High,
        "a close edit at the same spot"
    );
    assert_eq!(first.source[0].text, "local speed = 10 * factor");
}

#[test]
fn the_first_commit_has_nowhere_deeper_to_go() {
    let f = test_fixtures::linear(1).unwrap();
    let root = f.oid("HEAD").unwrap();

    let report = search(&f, &query(&root, "file0.txt", 1, 1, 1, None));

    assert_eq!(report.candidates.len(), 1);
    assert_eq!(best(&report).kind, OriginKind::Appeared);
    assert!(best(&report).deeper.is_none());
}

#[test]
fn a_move_not_yet_committed_is_traced_against_head() {
    let f = test_fixtures::linear(1).unwrap();
    stage(
        &f,
        "donor.lua",
        &format!("print('donor header')\n{BLOCK}print('donor footer')\n"),
    );
    stage(
        &f,
        "story.lua",
        "print('story header')\nprint('story footer')\n",
    );
    let head = f.commit_staged(10, "write the donor").unwrap();
    f.write_file(
        "donor.lua",
        "print('donor header')\nprint('donor footer')\n",
    )
    .unwrap();
    f.write_file(
        "story.lua",
        &format!(
            "print('story header')\n{}print('story footer')\n",
            edited_block()
        ),
    )
    .unwrap();

    let report = search(
        &f,
        &query(
            "0000000000000000000000000000000000000000",
            "story.lua",
            2,
            7,
            3,
            Some((&head, "story.lua")),
        ),
    );

    assert_eq!(best(&report).kind, OriginKind::Moved);
    assert_eq!(best(&report).path, "donor.lua");
    assert_eq!(best(&report).rev, head);
}

#[test]
fn a_parent_is_found_even_when_the_query_does_not_name_one() {
    let m = moved_between_files(false);

    let report = search(&m.fixture, &query(&m.after, "story.lua", 2, 7, 4, None));

    assert_eq!(best(&report).path, "donor.lua");
}

#[test]
fn a_cancelled_search_returns_nothing() {
    let m = moved_between_files(false);

    let report = open(&m.fixture)
        .origin_candidates(
            &query(
                &m.after,
                "story.lua",
                2,
                7,
                4,
                Some((&m.before, "story.lua")),
            ),
            &|| true,
        )
        .unwrap();

    assert!(report.is_none());
}

#[test]
fn the_origin_view_gets_the_source_lines_with_what_matched() {
    let m = moved_between_files(false);

    let report = search(
        &m.fixture,
        &query(
            &m.after,
            "story.lua",
            2,
            7,
            4,
            Some((&m.before, "story.lua")),
        ),
    );

    let found = best(&report);
    assert_eq!(found.source.len(), 6);
    assert_eq!(found.source[0].line, 2);
    assert_eq!(found.source[0].text, "local function rotate(cat, angle)");
    let changed = |status: &LineMatch| *status == LineMatch::Changed;
    assert_eq!(
        found
            .source
            .iter()
            .filter(|line| changed(&line.status))
            .count(),
        1
    );
    assert!(
        found
            .source
            .iter()
            .all(|line| line.status != LineMatch::Missing)
    );
    assert_eq!(found.block.len(), 6);
    assert_eq!(
        found.block.iter().filter(|status| changed(status)).count(),
        1
    );
    assert_eq!(found.block[3], LineMatch::Changed);
}

#[test]
fn a_range_outside_the_file_is_a_typed_error() {
    let m = moved_between_files(false);

    let failed = open(&m.fixture)
        .origin_candidates(&query(&m.after, "story.lua", 20, 30, 25, None), &|| false);

    assert!(failed.is_err());
}
