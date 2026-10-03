#![allow(clippy::unwrap_used, clippy::expect_used)]

//! What the Conflict Solver window is given for one file (M7, doc/08 §8, §13).

use app_state::{AppState, RepoId};
use diff_engine::SolverKind;

fn opened(f: &test_fixtures::Fixture) -> (AppState, RepoId) {
    let state = AppState::new();
    let summary = state.open_repository(f.path()).unwrap();
    (state, summary.repo)
}

fn kinds(data: &app_state::SolverData) -> Vec<SolverKind> {
    data.regions.iter().map(|region| region.kind).collect()
}

#[test]
fn a_both_modified_file_comes_with_regions_and_the_names_of_its_sides() {
    let f = test_fixtures::conflicted().unwrap();
    let (state, repo) = opened(&f);

    let data = state.solver_data(repo, "conflict.txt").unwrap();

    assert!(kinds(&data).contains(&SolverKind::Conflict));
    assert_eq!(data.context.ours, "main");
    assert_eq!(data.context.theirs, "dev");
    assert!(!data.binary && !data.too_large);
    assert!(!data.missing_ours && !data.missing_theirs);
    assert_eq!(data.base.as_deref(), Some("original line\n"));
    assert_eq!(data.ours.as_deref(), Some("changed by main\n"));
    assert_eq!(data.theirs.as_deref(), Some("changed by dev\n"));
}

#[test]
fn the_stages_the_solver_was_given_are_the_ones_in_the_index() {
    let f = test_fixtures::conflicted().unwrap();
    let (state, repo) = opened(&f);

    let data = state.solver_data(repo, "conflict.txt").unwrap();

    let listed = f.git(&["ls-files", "-u", "conflict.txt"]).unwrap();
    let oid = |stage: &str| {
        let line = listed
            .lines()
            .find(|line| line.contains(&format!(" {stage}\t")))
            .unwrap();
        line.split_whitespace().nth(1).unwrap().to_owned()
    };
    assert_eq!(data.stages.base, Some(oid("1")));
    assert_eq!(data.stages.ours, Some(oid("2")));
    assert_eq!(data.stages.theirs, Some(oid("3")));
    let stale = git_engine::ConflictStages {
        ours: Some("0".repeat(40)),
        ..data.stages.clone()
    };
    assert!(
        state
            .resolve_conflict_text(repo, "conflict.txt", "x\n", Some(&stale))
            .is_err()
    );
    assert_eq!(state.conflicted_paths(repo).unwrap(), ["conflict.txt"]);
}

#[test]
fn a_file_that_is_not_conflicted_is_an_error() {
    let f = test_fixtures::conflicted().unwrap();
    let (state, repo) = opened(&f);

    assert!(state.solver_data(repo, "nothing.txt").is_err());
}

const PLATE: &str = "src/device/src/plate/plate_navboard2.cpp";

#[test]
fn a_modify_delete_file_names_the_side_that_lost_it_and_has_no_regions() {
    let f = test_fixtures::linear(1).unwrap();
    f.commit_file(10, PLATE, "int a;\n").unwrap();
    f.git(&["switch", "-c", "theirs"]).unwrap();
    f.commit_file(11, PLATE, "int a = 1;\n").unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.git(&["rm", "-q", "--", PLATE]).unwrap();
    f.commit_staged(12, "drop the plate").unwrap();
    let _ = f.git(&["merge", "theirs"]);
    let (state, repo) = opened(&f);

    let data = state.solver_data(repo, PLATE).unwrap();

    assert!(data.missing_ours && !data.missing_theirs);
    assert!(data.regions.is_empty());
    assert_eq!(data.ours, None);
    assert_eq!(data.theirs.as_deref(), Some("int a = 1;\n"));
    assert_eq!(data.context.theirs, "theirs");
}

#[test]
fn add_add_is_one_conflict_without_a_base() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["switch", "-c", "theirs"]).unwrap();
    f.commit_file(10, "new.txt", "theirs\n").unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.commit_file(11, "new.txt", "ours\n").unwrap();
    let _ = f.git(&["merge", "theirs"]);
    let (state, repo) = opened(&f);

    let data = state.solver_data(repo, "new.txt").unwrap();

    assert!(!data.has_base);
    assert_eq!(kinds(&data), [SolverKind::Conflict]);
}

#[test]
fn a_binary_conflict_sends_no_text() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("pic.bin"), [0u8, 1, 2]).unwrap();
    f.git(&["add", "--", "pic.bin"]).unwrap();
    f.commit_staged(10, "add pic").unwrap();
    f.git(&["switch", "-c", "theirs"]).unwrap();
    std::fs::write(f.path().join("pic.bin"), [0u8, 9, 9]).unwrap();
    f.git(&["add", "--", "pic.bin"]).unwrap();
    f.commit_staged(11, "theirs pic").unwrap();
    f.git(&["switch", "main"]).unwrap();
    std::fs::write(f.path().join("pic.bin"), [0u8, 7, 7]).unwrap();
    f.git(&["add", "--", "pic.bin"]).unwrap();
    f.commit_staged(12, "ours pic").unwrap();
    let _ = f.git(&["merge", "theirs"]);
    let (state, repo) = opened(&f);

    let data = state.solver_data(repo, "pic.bin").unwrap();

    assert!(data.binary);
    assert!(data.regions.is_empty());
    assert_eq!((data.base, data.ours, data.theirs), (None, None, None));
}

#[test]
fn a_file_over_the_limit_is_not_merged_line_by_line() {
    let f = test_fixtures::linear(1).unwrap();
    let big = "x".repeat(app_state::MAX_SOLVER_BYTES + 1);
    f.commit_file(10, "big.txt", &format!("{big}\nbase\n"))
        .unwrap();
    f.git(&["switch", "-c", "theirs"]).unwrap();
    f.commit_file(11, "big.txt", &format!("{big}\ntheirs\n"))
        .unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.commit_file(12, "big.txt", &format!("{big}\nours\n"))
        .unwrap();
    let _ = f.git(&["merge", "theirs"]);
    let (state, repo) = opened(&f);

    let data = state.solver_data(repo, "big.txt").unwrap();

    assert!(data.too_large && !data.binary);
    assert!(data.regions.is_empty() && data.ours.is_none());
}

#[test]
fn a_crlf_file_is_given_with_lf_lines_and_remembered_as_crlf() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["config", "core.autocrlf", "false"]).unwrap();
    f.commit_file(10, "win.txt", "a\r\nb\r\n").unwrap();
    f.git(&["switch", "-c", "theirs"]).unwrap();
    f.commit_file(11, "win.txt", "a\r\ntheirs\r\n").unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.commit_file(12, "win.txt", "a\r\nours\r\n").unwrap();
    let _ = f.git(&["merge", "theirs"]);
    let (state, repo) = opened(&f);

    let data = state.solver_data(repo, "win.txt").unwrap();

    assert!(data.crlf);
    assert_eq!(data.ours.as_deref(), Some("a\nours\n"));
}

#[test]
fn a_file_settled_outside_is_marked_resolved_and_can_be_undone() {
    let f = test_fixtures::conflicted().unwrap();
    let (state, repo) = opened(&f);
    std::fs::write(f.path().join("conflict.txt"), "settled elsewhere\n").unwrap();

    state.mark_conflict_resolved(repo, "conflict.txt").unwrap();

    assert!(state.conflicted_paths(repo).unwrap().is_empty());
    assert!(
        f.git(&["status", "--short"])
            .unwrap()
            .contains("M  conflict.txt")
    );
}

#[test]
fn marking_a_file_that_is_not_conflicted_is_refused() {
    let f = test_fixtures::conflicted().unwrap();
    let (state, repo) = opened(&f);

    assert!(state.mark_conflict_resolved(repo, "nope.txt").is_err());
}

fn tool_writing(text: &str) -> (&'static str, String) {
    if cfg!(windows) {
        (
            "powershell",
            format!(
                "-NoProfile -Command \"Set-Content -LiteralPath '{{result}}' -Value '{text}'\""
            ),
        )
    } else {
        (
            "sh",
            format!("-c \"printf '%s\n' '{text}' > '{{result}}'\""),
        )
    }
}

fn tool_touching_nothing() -> (&'static str, String) {
    if cfg!(windows) {
        ("powershell", "-NoProfile -Command \"exit 0\"".to_owned())
    } else {
        ("sh", "-c \"exit 0\"".to_owned())
    }
}

fn run_tool(
    state: &AppState,
    repo: RepoId,
    path: &str,
    (program, args): (&str, String),
    temp: &std::path::Path,
) -> app_state::MergeToolOutcome {
    let (sender, receiver) = std::sync::mpsc::channel();
    state
        .start_merge_tool(repo, path, program, &args, temp, move |outcome| {
            sender.send(outcome).unwrap();
        })
        .unwrap();
    receiver
        .recv_timeout(std::time::Duration::from_secs(60))
        .unwrap()
}

#[test]
fn a_tool_that_writes_the_result_leaves_no_markers_and_the_file_waits_for_marking() {
    let f = test_fixtures::conflicted().unwrap();
    let (state, repo) = opened(&f);
    let temp = tempfile::tempdir().unwrap();

    let outcome = run_tool(
        &state,
        repo,
        "conflict.txt",
        tool_writing("merged by the tool"),
        temp.path(),
    );

    assert_eq!(outcome.exit_code, Some(0));
    assert!(!outcome.canceled && !outcome.markers_left);
    assert!(outcome.conflicted, "git still lists it until it is marked");
    assert!(
        std::fs::read_to_string(f.path().join("conflict.txt"))
            .unwrap()
            .contains("merged by the tool")
    );
    assert!(state.merge_tools_running(repo).is_empty());
    let left: Vec<_> = std::fs::read_dir(temp.path().join("cogit-merge"))
        .unwrap()
        .collect();
    assert!(left.is_empty(), "the temp files are removed: {left:?}");

    state.mark_conflict_resolved(repo, "conflict.txt").unwrap();
    assert!(state.conflicted_paths(repo).unwrap().is_empty());
}

#[test]
fn a_tool_that_leaves_the_markers_keeps_the_file_conflicted() {
    let f = test_fixtures::conflicted().unwrap();
    let (state, repo) = opened(&f);
    let temp = tempfile::tempdir().unwrap();

    let outcome = run_tool(
        &state,
        repo,
        "conflict.txt",
        tool_touching_nothing(),
        temp.path(),
    );

    assert!(outcome.markers_left && outcome.conflicted);
}

#[test]
fn with_no_program_and_no_merge_tool_in_git_the_launch_is_refused_with_a_reason() {
    let f = test_fixtures::conflicted().unwrap();
    let (state, repo) = opened(&f);
    let temp = tempfile::tempdir().unwrap();

    let err = state
        .start_merge_tool(repo, "conflict.txt", "", "", temp.path(), |_| {})
        .unwrap_err();

    assert!(err.to_string().contains("no merge tool"), "{err}");
}

#[test]
fn a_file_that_is_not_conflicted_gets_no_tool() {
    let f = test_fixtures::conflicted().unwrap();
    let (state, repo) = opened(&f);
    let temp = tempfile::tempdir().unwrap();

    assert!(
        state
            .start_merge_tool(repo, "nope.txt", "tool", "", temp.path(), |_| {})
            .is_err()
    );
}

#[test]
fn a_missing_program_is_reported_and_leaves_no_temp_files() {
    let f = test_fixtures::conflicted().unwrap();
    let (state, repo) = opened(&f);
    let temp = tempfile::tempdir().unwrap();

    let err = state
        .start_merge_tool(
            repo,
            "conflict.txt",
            "cogit-no-such-merge-tool",
            "{result}",
            temp.path(),
            |_| {},
        )
        .unwrap_err();

    assert!(
        err.to_string().contains("cogit-no-such-merge-tool"),
        "{err}"
    );
    let left = std::fs::read_dir(temp.path().join("cogit-merge"))
        .map(|dir| dir.count())
        .unwrap_or(0);
    assert_eq!(left, 0);
}

fn crlf_conflict(ours_tail: &str) -> test_fixtures::Fixture {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["config", "core.autocrlf", "false"]).unwrap();
    f.commit_file(10, "win.txt", "a\r\nb\r\n").unwrap();
    f.git(&["switch", "-c", "theirs"]).unwrap();
    f.commit_file(11, "win.txt", "a\r\ntheirs\r\n").unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.commit_file(12, "win.txt", &format!("a\r\nours{ours_tail}"))
        .unwrap();
    let _ = f.git(&["merge", "theirs"]);
    f
}

#[test]
fn a_saved_resolution_keeps_the_crlf_of_the_file() {
    let f = crlf_conflict("\r\n");
    let (state, repo) = opened(&f);

    state
        .resolve_conflict_text(repo, "win.txt", "a\nresolved\n", None)
        .unwrap();

    let bytes = std::fs::read(f.path().join("win.txt")).unwrap();
    assert_eq!(bytes, b"a\r\nresolved\r\n");
    assert!(state.conflicted_paths(repo).unwrap().is_empty());
}

#[test]
fn a_saved_resolution_keeps_a_file_without_a_final_newline_as_it_was() {
    let f = test_fixtures::linear(1).unwrap();
    f.commit_file(10, "bare.txt", "a\nb").unwrap();
    f.git(&["switch", "-c", "theirs"]).unwrap();
    f.commit_file(11, "bare.txt", "a\ntheirs").unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.commit_file(12, "bare.txt", "a\nours").unwrap();
    let _ = f.git(&["merge", "theirs"]);
    let (state, repo) = opened(&f);

    state
        .resolve_conflict_text(repo, "bare.txt", "a\nresolved\n", None)
        .unwrap();

    assert_eq!(
        std::fs::read(f.path().join("bare.txt")).unwrap(),
        b"a\nresolved"
    );
}

#[test]
fn a_resolution_written_with_conflict_markers_is_still_staged() {
    let f = test_fixtures::conflicted().unwrap();
    let (state, repo) = opened(&f);
    let text = "<<<<<<< main\nchanged by main\n=======\nchanged by dev\n>>>>>>> dev\n";

    state
        .resolve_conflict_text(repo, "conflict.txt", text, None)
        .unwrap();

    assert!(state.conflicted_paths(repo).unwrap().is_empty());
    assert_eq!(
        std::fs::read_to_string(f.path().join("conflict.txt")).unwrap(),
        text
    );
}

#[test]
fn a_modify_delete_file_kept_with_edited_text_is_written_and_staged() {
    let f = test_fixtures::linear(1).unwrap();
    f.commit_file(10, PLATE, "int a;\n").unwrap();
    f.git(&["switch", "-c", "theirs"]).unwrap();
    f.commit_file(11, PLATE, "int a = 1;\n").unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.git(&["rm", "-q", "--", PLATE]).unwrap();
    f.commit_staged(12, "drop the plate").unwrap();
    let _ = f.git(&["merge", "theirs"]);
    let (state, repo) = opened(&f);

    state
        .resolve_conflict_text(repo, PLATE, "int a = 2;\n", None)
        .unwrap();

    assert!(state.conflicted_paths(repo).unwrap().is_empty());
    assert_eq!(
        std::fs::read_to_string(f.path().join(PLATE)).unwrap(),
        "int a = 2;\n"
    );
    assert!(f.git(&["status", "--short"]).unwrap().contains("A  "));
}
