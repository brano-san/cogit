// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The Investigate window's whole walk, as the frontend drives it: blame the working
//! tree, take the picked line's block, search its origin, go deeper, search again.

use git_engine::{BlameReport, OriginKind, OriginQuery, RepoHandle};

const BLOCK: &str = "local function rotate(cat, angle)\n\
                     local radians = math.rad(angle)\n\
                     cat.x = cat.x * math.cos(radians)\n\
                     cat.y = cat.y * math.sin(radians)\n\
                     return cat\n\
                     end\n";

fn stage(f: &test_fixtures::Fixture, name: &str, contents: &str) {
    f.write_file(name, contents).unwrap();
    f.git(&["add", "--", name]).unwrap();
}

/// The query the frontend builds (`originQuery` in `lib/investigate/blame.ts`): the run
/// of lines around the pick with the same source and consecutive source lines.
fn query_for(report: &BlameReport, index: usize) -> OriginQuery {
    let lines = &report.lines;
    let joined = |a: usize, b: usize| {
        lines[a].source == lines[b].source && lines[b].orig_line == lines[a].orig_line + 1
    };
    let mut start = index;
    while start > 0 && joined(start - 1, start) {
        start -= 1;
    }
    let mut end = index + 1;
    while end < lines.len() && joined(end - 1, end) {
        end += 1;
    }
    let source = &report.sources[lines[start].source as usize];
    OriginQuery {
        commit: report.commits[source.commit as usize].oid.clone(),
        path: source.path.clone(),
        from: lines[start].orig_line,
        to: lines[end - 1].orig_line,
        line: lines[index].orig_line,
        previous: source.previous.clone(),
    }
}

#[test]
fn a_block_moved_into_a_renamed_file_is_traced_to_its_first_version() {
    let f = test_fixtures::linear(1).unwrap();
    stage(
        &f,
        "src/donor.lua",
        &format!("print('donor header')\n{BLOCK}print('donor footer')\n"),
    );
    stage(
        &f,
        "src/story.lua",
        "local story = {}\nstory.title = 'The cat'\nreturn story\n",
    );
    let written = f
        .commit_staged(10, "write the donor and the story")
        .unwrap();
    f.git(&["mv", "src/story.lua", "src/tale.lua"]).unwrap();
    let renamed = f.commit_staged(11, "rename the story").unwrap();
    stage(
        &f,
        "src/donor.lua",
        "print('donor header')\nprint('donor footer')\n",
    );
    let moved: String = BLOCK
        .replace("math.sin(radians)", "math.sin(radians) + 1")
        .lines()
        .map(|line| format!("  {line}\n"))
        .collect();
    stage(
        &f,
        "src/tale.lua",
        &format!("local story = {{}}\nstory.title = 'The cat'\n{moved}return story\n"),
    );
    let brought = f.commit_staged(12, "bring rotate into the tale").unwrap();
    f.write_file(
        "src/tale.lua",
        &format!(
            "local story = {{}}\nstory.title = 'The cat'\n{moved}return story\nprint('wip')\n"
        ),
    )
    .unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    let log = repo.file_log("src/tale.lua", None, true, 100).unwrap();
    let oids: Vec<&str> = log.iter().map(|row| row.oid.as_str()).collect();
    assert_eq!(oids, [&brought, &renamed, &written]);

    let blame = repo.blame_origins("src/tale.lua", None, false).unwrap();
    let picked = 4;
    assert_eq!(
        blame.lines[picked].text.trim(),
        "cat.x = cat.x * math.cos(radians)"
    );
    let query = query_for(&blame, picked);
    assert_eq!(query.commit, brought);
    assert_eq!((query.from, query.to), (3, 8));

    let report = repo.origin_candidates(&query, &|| false).unwrap().unwrap();
    let best = &report.candidates[report.best as usize];
    assert_eq!(best.kind, OriginKind::Moved);
    assert_eq!(best.path, "src/donor.lua");
    let deeper = best.deeper.clone().unwrap();
    assert_eq!(deeper.rev, renamed, "the version just before the move");
    assert_eq!((deeper.path.as_str(), deeper.line), ("src/donor.lua", 4));

    let deeper_blame = repo
        .blame_origins(&deeper.path, Some(&deeper.rev), false)
        .unwrap();
    let again = query_for(&deeper_blame, deeper.line as usize - 1);
    assert_eq!(again.commit, written);
    let last = repo.origin_candidates(&again, &|| false).unwrap().unwrap();
    let origin = &last.candidates[last.best as usize];
    assert_eq!(origin.kind, OriginKind::Appeared, "{last:#?}");
}
