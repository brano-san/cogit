#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, RepoId, SplitBlock, SplitPart};

const LINES: &str = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n";
const MESSAGE: &str = "two edits and a binary\ncontinued first paragraph\n\nDetails of the change.";

fn two_hunks_and_a_binary() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("lines.txt", LINES).unwrap();
    f.git(&["add", "--", "lines.txt"]).unwrap();
    f.commit_staged(2, "add lines").unwrap();
    f.write_file(
        "lines.txt",
        &LINES
            .replace("\n2\n", "\ntwo\n")
            .replace("19\n", "nineteen\n"),
    )
    .unwrap();
    std::fs::write(f.path().join("blob.bin"), [0u8, 1, 2, 0, 255]).unwrap();
    f.git(&["add", "--", "lines.txt", "blob.bin"]).unwrap();
    f.commit_staged(3, MESSAGE).unwrap();
    f
}

fn open(f: &test_fixtures::Fixture) -> (AppState, RepoId) {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    (state, repo)
}

fn part(message: &str, blocks: &[(&str, Option<u32>)]) -> SplitPart {
    SplitPart {
        message: message.to_owned(),
        blocks: blocks
            .iter()
            .map(|(path, hunk)| SplitBlock {
                path: (*path).to_owned(),
                hunk: *hunk,
            })
            .collect(),
    }
}

fn tree_of(f: &test_fixtures::Fixture, rev: &str) -> String {
    f.oid(&format!("{rev}^{{tree}}")).unwrap()
}

#[test]
fn a_text_file_comes_in_hunks_and_a_binary_whole() {
    let f = two_hunks_and_a_binary();
    let (state, repo) = open(&f);

    let preview = state.split_preview(repo, "HEAD").unwrap();
    assert_eq!(preview.message.trim_end(), MESSAGE);
    let files = preview.files;

    let lines = files.iter().find(|file| file.path == "lines.txt").unwrap();
    assert_eq!(lines.blocks.len(), 2);
    assert!(!lines.whole);
    let blob = files.iter().find(|file| file.path == "blob.bin").unwrap();
    assert!(blob.whole);
    assert!(
        blob.note.as_deref().unwrap_or_default().contains("binary"),
        "{blob:?}"
    );
}

#[test]
fn hunks_of_one_file_land_in_different_commits() {
    let f = two_hunks_and_a_binary();
    let tree = tree_of(&f, "HEAD");
    let (state, repo) = open(&f);

    state
        .split_commit(
            repo,
            "HEAD",
            &[
                part("second hunk", &[("lines.txt", Some(1))]),
                part("binary", &[("blob.bin", None)]),
                part("first hunk", &[]),
            ],
        )
        .unwrap();

    assert_eq!(tree_of(&f, "HEAD"), tree);
    let early = f.git(&["show", "HEAD~2:lines.txt"]).unwrap();
    assert!(
        early.contains("nineteen") && early.contains("\n2\n"),
        "{early}"
    );
    assert!(f.git(&["show", "HEAD~2:blob.bin"]).is_err());
    assert!(f.git(&["show", "HEAD~1:blob.bin"]).is_ok());
    assert_eq!(
        f.git(&["log", "--format=%s", "-3"]).unwrap(),
        "first hunk\nbinary\nsecond hunk\n"
    );
}

#[test]
fn undo_puts_the_branch_back_on_the_original_commit() {
    let f = two_hunks_and_a_binary();
    let original = f.oid("HEAD").unwrap();
    let (state, repo) = open(&f);
    f.write_file("lines.txt", "staged edit\n").unwrap();
    f.git(&["add", "--", "lines.txt"]).unwrap();
    f.write_file("lines.txt", "unstaged edit\n").unwrap();
    let index = f.git(&["rev-parse", "--git-path", "index"]).unwrap();
    let index_path = f.path().join(index.trim());
    let index = std::fs::read(&index_path).unwrap();
    state
        .split_commit(
            repo,
            "HEAD",
            &[part("one", &[("lines.txt", Some(0))]), part("two", &[])],
        )
        .unwrap();
    assert_eq!(std::fs::read(&index_path).unwrap(), index);
    let log = state.safety_log();
    assert!(log[0].undoable, "{log:?}");
    assert!(log[0].description.contains("into 2 commits"), "{log:?}");

    state.undo_last(repo).unwrap();

    assert_eq!(f.oid("HEAD").unwrap(), original);
    assert_eq!(std::fs::read(index_path).unwrap(), index);
    assert_eq!(
        std::fs::read_to_string(f.path().join("lines.txt")).unwrap(),
        "unstaged edit\n"
    );
}

#[test]
fn undo_refuses_once_the_branch_moves() {
    let f = two_hunks_and_a_binary();
    let (state, repo) = open(&f);
    state
        .split_commit(
            repo,
            "HEAD",
            &[part("one", &[("blob.bin", None)]), part("two", &[])],
        )
        .unwrap();
    let moved = f.commit_file(4, "later.txt", "later\n").unwrap();

    assert!(state.undo_last(repo).is_err());
    assert_eq!(f.oid("HEAD").unwrap(), moved);
}

#[test]
fn a_block_given_to_two_commits_is_refused() {
    let f = two_hunks_and_a_binary();
    let head = f.oid("HEAD").unwrap();
    let (state, repo) = open(&f);

    let result = state.split_commit(
        repo,
        "HEAD",
        &[
            part("one", &[("lines.txt", Some(0))]),
            part("two", &[("lines.txt", None)]),
        ],
    );

    assert!(result.is_err());
    assert_eq!(f.oid("HEAD").unwrap(), head);
}

#[test]
fn a_commit_on_a_protected_remote_branch_is_refused() {
    let f = test_fixtures::with_remote().unwrap();
    let head = f.oid("HEAD").unwrap();
    let (state, repo) = open(&f);
    let files = state.split_preview(repo, "HEAD~2").unwrap().files;
    let first = files[0].path.clone();

    let result = state.split_commit(
        repo,
        "HEAD~2",
        &[part("one", &[(&first, None)]), part("two", &[])],
    );

    let message = format!("{:?}", result.unwrap_err());
    assert!(
        message.contains("rewrite what others already have"),
        "{message}"
    );
    assert_eq!(f.oid("HEAD").unwrap(), head);
}
