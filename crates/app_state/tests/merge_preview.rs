#![allow(clippy::unwrap_used, clippy::expect_used)]

//! What the four-panel merge view is given: the three sides already merged into regions,
//! so the UI counts conflicts rather than parsing markers out of a file (M7, doc/08 §8).

use app_state::AppState;

fn opened(f: &test_fixtures::Fixture) -> (AppState, app_state::RepoId) {
    let state = AppState::new();
    let summary = state.open_repository(f.path()).unwrap();
    (state, summary.repo)
}

#[test]
fn a_conflicted_file_comes_back_as_regions() {
    let f = test_fixtures::conflicted().unwrap();
    let (state, repo) = opened(&f);

    let regions = state.merge_preview(repo, "conflict.txt").unwrap();

    assert!(!regions.is_empty());
    assert!(regions.iter().any(diff_engine::Region::is_conflict));
}

#[test]
fn a_file_that_is_not_conflicted_is_an_error_rather_than_an_empty_merge() {
    let f = test_fixtures::conflicted().unwrap();
    let (state, repo) = opened(&f);
    assert!(state.merge_preview(repo, "no-such-file.txt").is_err());
}

#[test]
fn an_unknown_repo_id_is_an_error() {
    let state = AppState::new();
    assert!(
        state
            .merge_preview(app_state::RepoId(404), "a.txt")
            .is_err()
    );
}

#[test]
fn resolving_with_the_merged_text_clears_the_conflict() {
    let f = test_fixtures::conflicted().unwrap();
    let (state, repo) = opened(&f);

    let regions = state.merge_preview(repo, "conflict.txt").unwrap();
    let text: String = regions
        .iter()
        .flat_map(|region| match region {
            diff_engine::Region::Clean { lines, .. } => lines.clone(),
            diff_engine::Region::Conflict { ours, .. } => ours.clone(),
        })
        .collect::<Vec<_>>()
        .join("\n");

    state
        .resolve_conflict_text(repo, "conflict.txt", &format!("{text}\n"))
        .unwrap();

    assert!(state.conflicted_paths(repo).unwrap().is_empty());
}

/// A conflict on `path` whose sides are raw bytes; `None` deletes the file on that side.
fn conflict_of(
    path: &str,
    base: &[u8],
    ours: Option<&[u8]>,
    theirs: Option<&[u8]>,
) -> test_fixtures::Fixture {
    let f = test_fixtures::empty().unwrap();
    let write = |bytes: Option<&[u8]>, index: i64| {
        match bytes {
            Some(bytes) => {
                std::fs::write(f.path().join(path), bytes).unwrap();
                f.git(&["add", "--", path]).unwrap();
            }
            None => {
                f.git(&["rm", "-q", "--", path]).unwrap();
            }
        }
        f.commit_staged(index, &format!("side {index}")).unwrap();
    };
    write(Some(base), 0);
    f.git(&["switch", "-q", "-c", "theirs"]).unwrap();
    write(theirs, 1);
    f.git(&["switch", "-q", "main"]).unwrap();
    write(ours, 2);
    let _ = f.git_at(3, &["merge", "--no-edit", "theirs"]);
    f
}

// Four panels of replacement characters, and Save wrote them over the bytes and staged
// the result: a binary or non-UTF-8 conflict is only ever taken whole, from one side.
#[test]
fn a_binary_conflict_is_not_merged_as_text() {
    let f = conflict_of(
        "pic.bin",
        b"base\0\x01",
        Some(b"ours\0\x02"),
        Some(b"theirs\0\x03"),
    );
    let (state, repo) = opened(&f);

    assert!(state.merge_preview(repo, "pic.bin").is_err());
    assert!(state.conflict_text(repo, "pic.bin").unwrap().binary);
}

#[test]
fn a_conflict_that_is_not_utf8_is_not_merged_as_text() {
    let f = conflict_of(
        "cp1251.txt",
        b"\xcf\xf0\xe8\xe2\xe5\xf2\n",
        Some(b"\xcf\xf0\xe8\xe2\xe5\xf2 1\n"),
        Some(b"\xcf\xf0\xe8\xe2\xe5\xf2 2\n"),
    );
    let (state, repo) = opened(&f);

    assert!(state.merge_preview(repo, "cp1251.txt").is_err());
}

#[test]
fn a_binary_conflict_refuses_a_text_resolution() {
    let f = conflict_of(
        "pic.bin",
        b"base\0\x01",
        Some(b"ours\0\x02"),
        Some(b"theirs\0\x03"),
    );
    let (state, repo) = opened(&f);

    let written = state.resolve_conflict_text(repo, "pic.bin", "ours\u{FFFD}\n");

    assert!(written.is_err(), "{written:?}");
    assert_eq!(state.conflicted_paths(repo).unwrap(), vec!["pic.bin"]);
}

#[test]
fn a_binary_conflict_is_still_resolved_by_taking_a_side_whole() {
    let f = conflict_of(
        "pic.bin",
        b"base\0\x01",
        Some(b"ours\0\x02"),
        Some(b"theirs\0\x03"),
    );
    let (state, repo) = opened(&f);

    state
        .resolve_conflict(repo, "pic.bin", git_engine::ConflictSide::Theirs)
        .unwrap();

    assert_eq!(
        std::fs::read(f.path().join("pic.bin")).unwrap(),
        b"theirs\0\x03"
    );
    assert!(state.conflicted_paths(repo).unwrap().is_empty());
}

// Deleted on our side, changed on theirs. The missing side read as an empty file, so the
// merge view offered "all lines deleted"; choosing it wrote an empty file and staged it,
// and the file stayed in the tree instead of going.
#[test]
fn a_file_deleted_on_one_side_is_not_merged_as_text() {
    let f = conflict_of("f.txt", b"base\n", None, Some(b"theirs\n"));
    let (state, repo) = opened(&f);

    assert!(state.merge_preview(repo, "f.txt").is_err());
}

#[test]
fn taking_the_side_that_deleted_the_file_deletes_it() {
    let f = conflict_of("f.txt", b"base\n", None, Some(b"theirs\n"));
    let (state, repo) = opened(&f);

    state
        .resolve_conflict(repo, "f.txt", git_engine::ConflictSide::Ours)
        .unwrap();

    assert!(state.conflicted_paths(repo).unwrap().is_empty());
    assert_eq!(f.git(&["ls-files", "--", "f.txt"]).unwrap(), "");
    assert!(!f.path().join("f.txt").exists());
}

#[test]
fn taking_the_side_that_kept_the_file_keeps_it() {
    let f = conflict_of("f.txt", b"base\n", Some(b"ours\n"), None);
    let (state, repo) = opened(&f);

    state
        .resolve_conflict(repo, "f.txt", git_engine::ConflictSide::Ours)
        .unwrap();

    assert!(state.conflicted_paths(repo).unwrap().is_empty());
    assert_eq!(std::fs::read(f.path().join("f.txt")).unwrap(), b"ours\n");
}
