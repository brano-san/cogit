// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{FileStatus, RepoHandle};

fn open(fixture: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(fixture.path()).unwrap()
}

fn staged(repo: &RepoHandle) -> Vec<String> {
    repo.worktree_files()
        .unwrap()
        .staged
        .into_iter()
        .map(|f| f.path)
        .collect()
}

fn unstaged(repo: &RepoHandle) -> Vec<String> {
    repo.worktree_files()
        .unwrap()
        .unstaged
        .into_iter()
        .map(|f| f.path)
        .collect()
}

#[test]
fn staging_moves_a_file_from_unstaged_to_staged() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("file0.txt"), "edited\n").unwrap();
    let repo = open(&f);

    repo.stage(&["file0.txt".to_owned()]).unwrap();

    assert_eq!(staged(&repo), ["file0.txt"]);
    assert!(unstaged(&repo).is_empty());
}

#[test]
fn staging_an_untracked_file_adds_it() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("fresh.txt"), "new\n").unwrap();
    let repo = open(&f);

    repo.stage(&["fresh.txt".to_owned()]).unwrap();

    let files = repo.worktree_files().unwrap();
    assert_eq!(files.staged[0].status, FileStatus::Added);
}

#[test]
fn staging_a_deletion_records_the_removal() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::remove_file(f.path().join("file0.txt")).unwrap();
    let repo = open(&f);

    repo.stage(&["file0.txt".to_owned()]).unwrap();

    let files = repo.worktree_files().unwrap();
    assert_eq!(files.staged[0].status, FileStatus::Deleted);
    assert!(files.unstaged.is_empty());
}

#[test]
fn unstaging_moves_a_file_back() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("file0.txt"), "edited\n").unwrap();
    f.git(&["add", "--", "file0.txt"]).unwrap();
    let repo = open(&f);

    repo.unstage(&["file0.txt".to_owned()]).unwrap();

    assert!(staged(&repo).is_empty());
    assert_eq!(unstaged(&repo), ["file0.txt"]);
}

#[test]
fn unstaging_a_new_file_leaves_it_untracked_not_deleted() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("fresh.txt"), "new\n").unwrap();
    f.git(&["add", "--", "fresh.txt"]).unwrap();
    let repo = open(&f);

    repo.unstage(&["fresh.txt".to_owned()]).unwrap();

    let files = repo.worktree_files().unwrap();
    assert_eq!(files.unstaged[0].status, FileStatus::Untracked);
    assert!(
        f.path().join("fresh.txt").exists(),
        "unstaging must never touch the file on disk"
    );
}

#[test]
fn unstaging_on_an_unborn_head_still_works() {
    let f = test_fixtures::empty().unwrap();
    std::fs::write(f.path().join("first.txt"), "content\n").unwrap();
    f.git(&["add", "--", "first.txt"]).unwrap();
    let repo = open(&f);

    repo.unstage(&["first.txt".to_owned()]).unwrap();

    assert!(
        staged(&repo).is_empty(),
        "INV-07: no HEAD is a normal state"
    );
}

#[test]
fn discarding_restores_the_file_from_the_index() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("file0.txt"), "unwanted\n").unwrap();
    let repo = open(&f);

    repo.discard(&["file0.txt".to_owned()]).unwrap();

    assert_eq!(
        std::fs::read_to_string(f.path().join("file0.txt")).unwrap(),
        "content 0\n"
    );
    assert!(unstaged(&repo).is_empty());
}

#[test]
fn discarding_deletes_an_untracked_file() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("scratch.txt"), "junk\n").unwrap();
    let repo = open(&f);

    repo.discard(&["scratch.txt".to_owned()]).unwrap();

    assert!(!f.path().join("scratch.txt").exists());
}

#[test]
fn discarding_brings_back_a_file_deleted_from_disk() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::remove_file(f.path().join("file0.txt")).unwrap();
    let repo = open(&f);

    repo.discard(&["file0.txt".to_owned()]).unwrap();

    assert!(f.path().join("file0.txt").exists());
}

#[test]
fn several_paths_are_handled_in_one_call() {
    let f = test_fixtures::linear(3).unwrap();
    for name in ["file0.txt", "file1.txt", "file2.txt"] {
        std::fs::write(f.path().join(name), "touched\n").unwrap();
    }
    let repo = open(&f);

    repo.stage(&[
        "file0.txt".to_owned(),
        "file1.txt".to_owned(),
        "file2.txt".to_owned(),
    ])
    .unwrap();

    assert_eq!(staged(&repo).len(), 3);
}

#[test]
fn a_path_with_spaces_is_not_split() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("two words.txt"), "body\n").unwrap();
    let repo = open(&f);

    repo.stage(&["two words.txt".to_owned()]).unwrap();

    assert_eq!(staged(&repo), ["two words.txt"]);
}

#[test]
fn an_empty_path_list_is_refused_rather_than_staging_everything() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("file0.txt"), "edited\n").unwrap();
    let repo = open(&f);

    assert!(repo.stage(&[]).is_err());
    assert!(repo.unstage(&[]).is_err());
    assert!(repo.discard(&[]).is_err());
    assert_eq!(
        unstaged(&repo),
        ["file0.txt"],
        "an empty list must change nothing"
    );
}

#[test]
fn a_path_that_does_not_exist_reports_gits_own_words() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    let err = repo.stage(&["never-existed.txt".to_owned()]).unwrap_err();

    match err {
        git_engine::GitError::Command(details) => {
            assert!(details.stderr.contains("never-existed.txt"), "{details:?}");
        }
        other => panic!("expected a command failure, got {other:?}"),
    }
}

/// Longer together than the 32 767 characters Windows allows a whole command line. At the
/// root: an untracked directory is listed as one entry, never file by file.
fn many_untracked(f: &test_fixtures::Fixture) -> Vec<String> {
    (0..1200)
        .map(|i| {
            let name = format!("a-file-whose-name-is-long-enough-to-matter-{i:04}.txt");
            std::fs::write(f.path().join(&name), "x\n").unwrap();
            name
        })
        .collect()
}

#[test]
fn a_list_longer_than_a_command_line_is_staged_and_unstaged_whole() {
    let f = test_fixtures::linear(1).unwrap();
    let paths = many_untracked(&f);
    let repo = open(&f);

    repo.stage(&paths).unwrap();
    assert_eq!(staged(&repo).len(), paths.len());

    repo.unstage(&paths).unwrap();
    assert!(staged(&repo).is_empty());
}

#[test]
fn a_list_longer_than_a_command_line_is_discarded_whole() {
    let f = test_fixtures::linear(1).unwrap();
    let paths = many_untracked(&f);
    let repo = open(&f);

    repo.discard(&paths).unwrap();

    assert!(unstaged(&repo).is_empty());
}

#[test]
fn a_list_too_long_to_stash_is_refused_before_anything_changes() {
    let f = test_fixtures::linear(1).unwrap();
    let paths = many_untracked(&f);
    let repo = open(&f);

    assert!(repo.stash_paths(&paths, "many").is_err());

    assert!(repo.stashes().unwrap().is_empty());
    assert_eq!(unstaged(&repo).len(), paths.len());
}

// Paths went to git as pathspecs, where `[1]` is a character class: discarding
// `test[1].txt` also deleted the untracked `test1.txt` beside it.
#[test]
fn discarding_a_name_with_brackets_touches_only_that_file() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("test[1].txt"), "chosen\n").unwrap();
    std::fs::write(f.path().join("test1.txt"), "not chosen\n").unwrap();

    open(&f).discard(&["test[1].txt".to_owned()]).unwrap();

    assert!(!f.path().join("test[1].txt").exists());
    assert_eq!(
        std::fs::read_to_string(f.path().join("test1.txt")).unwrap(),
        "not chosen\n"
    );
}

#[test]
fn staging_a_name_with_brackets_stages_only_that_file() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("test[1].txt"), "chosen\n").unwrap();
    std::fs::write(f.path().join("test1.txt"), "not chosen\n").unwrap();
    let repo = open(&f);

    repo.stage(&["test[1].txt".to_owned()]).unwrap();

    let staged = f.git(&["diff", "--cached", "--name-only"]).unwrap();
    assert_eq!(staged.trim(), "test[1].txt");
}

// Stage all is one `git add --all`: no pathspec for git to match against every entry.
#[test]
fn staging_everything_takes_edits_deletions_and_new_files() {
    let f = test_fixtures::linear(3).unwrap();
    std::fs::write(f.path().join("file0.txt"), "edited\n").unwrap();
    std::fs::remove_file(f.path().join("file1.txt")).unwrap();
    std::fs::create_dir_all(f.path().join("new dir")).unwrap();
    for name in ["new dir/two words.txt", "-leading.txt", "ünïcødé.txt"] {
        std::fs::write(f.path().join(name), "fresh\n").unwrap();
    }
    let repo = open(&f);

    repo.stage_all().unwrap();

    assert_eq!(
        staged(&repo),
        [
            "-leading.txt",
            "file0.txt",
            "file1.txt",
            "new dir/two words.txt",
            "ünïcødé.txt"
        ]
    );
    assert!(unstaged(&repo).is_empty(), "{:?}", unstaged(&repo));
}

#[test]
fn staging_everything_leaves_ignored_files_alone() {
    let f = test_fixtures::linear(1).unwrap();
    f.commit_file(2, ".gitignore", "*.log\n").unwrap();
    std::fs::write(f.path().join("build.log"), "noise\n").unwrap();
    std::fs::write(f.path().join("file0.txt"), "edited\n").unwrap();
    let repo = open(&f);

    repo.stage_all().unwrap();

    assert_eq!(staged(&repo), ["file0.txt"]);
}

#[test]
fn staging_everything_reaches_into_nested_folders() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::create_dir_all(f.path().join("deep/er")).unwrap();
    std::fs::write(f.path().join("deep/er/inner.txt"), "in\n").unwrap();
    std::fs::write(f.path().join("file0.txt"), "edited\n").unwrap();
    let repo = open(&f);

    repo.stage_all().unwrap();

    assert_eq!(staged(&repo), ["deep/er/inner.txt", "file0.txt"]);
}
