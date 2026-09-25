#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The file context menus of the Files panel: Remove, Move or Rename, the two index
//! flags, the Index Editor, and taking one file's change out of a past commit.

use git_engine::{FileStatus, GitError, IndexFlag, RepoHandle, WorktreeView};

fn open(fixture: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(fixture.path()).unwrap()
}

fn paths(list: &[&str]) -> Vec<String> {
    list.iter().map(|path| (*path).to_owned()).collect()
}

fn status_of(repo: &RepoHandle, path: &str, view: WorktreeView) -> Option<(bool, FileStatus)> {
    let files = repo.worktree_files_with(view).unwrap();
    files
        .staged
        .iter()
        .map(|file| (true, file))
        .chain(files.unstaged.iter().map(|file| (false, file)))
        .find(|(_, file)| file.path == path)
        .map(|(staged, file)| (staged, file.status))
}

fn read(fixture: &test_fixtures::Fixture, path: &str) -> String {
    std::fs::read_to_string(fixture.path().join(path)).unwrap()
}

#[test]
fn remove_without_deleting_stops_tracking_and_keeps_the_file() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);

    repo.remove_from_repository(&paths(&["file0.txt"]), false)
        .unwrap();

    assert!(
        f.path().join("file0.txt").exists(),
        "git rm --cached keeps it"
    );
    assert_eq!(
        status_of(&repo, "file0.txt", WorktreeView::default()),
        Some((true, FileStatus::Deleted))
    );
    let untracked = repo.worktree_files().unwrap().unstaged;
    assert!(
        untracked
            .iter()
            .any(|file| file.path == "file0.txt" && file.status == FileStatus::Untracked)
    );
}

#[test]
fn remove_with_deleting_takes_the_file_off_the_disk_too() {
    let f = test_fixtures::linear(2).unwrap();
    let repo = open(&f);

    repo.remove_from_repository(&paths(&["file0.txt", "file1.txt"]), true)
        .unwrap();

    assert!(!f.path().join("file0.txt").exists());
    assert!(!f.path().join("file1.txt").exists());
    assert_eq!(
        status_of(&repo, "file1.txt", WorktreeView::default()),
        Some((true, FileStatus::Deleted))
    );
}

#[test]
fn remove_refuses_an_empty_list_rather_than_the_whole_repository() {
    let f = test_fixtures::linear(1).unwrap();
    let err = open(&f).remove_from_repository(&[], false).unwrap_err();
    assert!(matches!(err, GitError::InvalidState(_)), "{err:?}");
}

#[test]
fn moving_a_tracked_file_is_a_staged_rename() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    repo.move_path("file0.txt", "docs/renamed.txt").unwrap();

    assert!(!f.path().join("file0.txt").exists());
    assert_eq!(read(&f, "docs/renamed.txt"), "content 0\n");
    let staged = repo.worktree_files().unwrap().staged;
    assert!(
        staged
            .iter()
            .any(|file| file.path == "docs/renamed.txt" && file.status == FileStatus::Renamed),
        "{staged:?}"
    );
}

#[test]
fn moving_an_untracked_file_moves_it_on_disk() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("scratch.txt", "notes\n").unwrap();
    let repo = open(&f);

    repo.move_path("scratch.txt", "notes/scratch.md").unwrap();

    assert!(!f.path().join("scratch.txt").exists());
    assert_eq!(read(&f, "notes/scratch.md"), "notes\n");
}

#[test]
fn moving_onto_an_existing_file_is_refused() {
    let f = test_fixtures::linear(2).unwrap();
    let err = open(&f).move_path("file0.txt", "file1.txt").unwrap_err();
    assert!(matches!(err, GitError::InvalidState(_)), "{err:?}");
    assert_eq!(
        read(&f, "file1.txt"),
        "content 1\n",
        "nothing was overwritten"
    );
}

#[test]
fn assume_unchanged_hides_an_edit_until_it_is_cleared() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);
    let flagged = WorktreeView {
        assume_unchanged: true,
        ..WorktreeView::default()
    };

    repo.set_index_flag(&paths(&["file0.txt"]), IndexFlag::AssumeUnchanged, true)
        .unwrap();
    f.write_file("file0.txt", "edited\n").unwrap();
    assert_eq!(
        status_of(&repo, "file0.txt", flagged),
        Some((false, FileStatus::AssumeUnchanged))
    );

    repo.set_index_flag(&paths(&["file0.txt"]), IndexFlag::AssumeUnchanged, false)
        .unwrap();
    assert_eq!(
        status_of(&repo, "file0.txt", flagged),
        Some((false, FileStatus::Modified))
    );
}

#[test]
fn skip_worktree_is_its_own_flag() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);
    let skipped = WorktreeView {
        skipped: true,
        ..WorktreeView::default()
    };

    repo.set_index_flag(&paths(&["file0.txt"]), IndexFlag::SkipWorktree, true)
        .unwrap();
    assert_eq!(
        status_of(&repo, "file0.txt", skipped),
        Some((false, FileStatus::Skipped))
    );

    repo.set_index_flag(&paths(&["file0.txt"]), IndexFlag::SkipWorktree, false)
        .unwrap();
    assert_eq!(status_of(&repo, "file0.txt", skipped), None);
}

#[test]
fn the_index_editor_reads_all_three_versions() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("file0.txt", "staged\n").unwrap();
    f.git(&["add", "--", "file0.txt"]).unwrap();
    f.write_file("file0.txt", "on disk\r\n").unwrap();

    let sides = open(&f).index_editor_sides("file0.txt").unwrap();

    assert_eq!(sides.head.as_deref(), Some("content 0\n"));
    assert_eq!(sides.index.as_deref(), Some("staged\n"));
    assert_eq!(
        sides.worktree.as_deref(),
        Some("on disk\r\n"),
        "line endings kept"
    );
    assert!(!sides.binary);
}

#[test]
fn a_new_file_has_no_head_side_and_a_binary_one_no_text_at_all() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("fresh.txt", "new\n").unwrap();
    std::fs::write(f.path().join("blob.bin"), [0u8, 159, 146, 150]).unwrap();
    let repo = open(&f);

    let fresh = repo.index_editor_sides("fresh.txt").unwrap();
    assert_eq!(
        (fresh.head, fresh.index, fresh.worktree.as_deref()),
        (None, None, Some("new\n"))
    );

    let binary = repo.index_editor_sides("blob.bin").unwrap();
    assert!(binary.binary);
    assert_eq!(binary.worktree, None);
}

#[test]
fn writing_the_index_side_stages_exactly_that_text_and_leaves_the_disk_alone() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("file0.txt", "on disk\n").unwrap();
    let repo = open(&f);

    repo.write_index_text("file0.txt", "only in the index\n")
        .unwrap();

    let sides = repo.index_editor_sides("file0.txt").unwrap();
    assert_eq!(sides.index.as_deref(), Some("only in the index\n"));
    assert_eq!(read(&f, "file0.txt"), "on disk\n");
}

#[test]
fn writing_the_index_side_keeps_the_executable_bit() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["update-index", "--chmod=+x", "--", "file0.txt"])
        .unwrap();
    let repo = open(&f);

    repo.write_index_text("file0.txt", "changed\n").unwrap();

    let staged = f.git(&["ls-files", "--stage", "--", "file0.txt"]).unwrap();
    assert!(staged.starts_with("100755 "), "{staged}");
}

#[test]
fn writing_the_working_tree_side_changes_only_the_file() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    repo.write_worktree_text("file0.txt", "typed in\r\n")
        .unwrap();

    assert_eq!(read(&f, "file0.txt"), "typed in\r\n");
    let sides = repo.index_editor_sides("file0.txt").unwrap();
    assert_eq!(sides.index.as_deref(), Some("content 0\n"));
}

#[test]
fn save_as_writes_the_version_from_the_commit() {
    let f = test_fixtures::linear(2).unwrap();
    f.write_file("file0.txt", "later edit\n").unwrap();
    let repo = open(&f);
    let target = tempfile::tempdir().unwrap();
    let out = target.path().join("copy.txt");

    repo.save_blob("HEAD~1", "file0.txt", &out).unwrap();

    assert_eq!(std::fs::read_to_string(out).unwrap(), "content 0\n");
}

#[test]
fn save_as_of_a_path_the_commit_does_not_have_is_an_error() {
    let f = test_fixtures::linear(2).unwrap();
    let target = tempfile::tempdir().unwrap();
    let err = open(&f)
        .save_blob("HEAD~1", "file1.txt", &target.path().join("x"))
        .unwrap_err();
    assert!(matches!(err, GitError::InvalidState(_)), "{err:?}");
}

#[test]
fn a_version_opened_for_reading_is_a_read_only_copy_that_can_be_opened_again() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);
    let dir = tempfile::tempdir().unwrap();

    let first = repo
        .export_read_only("HEAD", "file0.txt", dir.path())
        .unwrap();
    assert_eq!(std::fs::read_to_string(&first).unwrap(), "content 0\n");
    assert!(std::fs::metadata(&first).unwrap().permissions().readonly());
    assert_eq!(first.file_name().unwrap(), "file0.txt");

    let again = repo
        .export_read_only("HEAD", "file0.txt", dir.path())
        .unwrap();
    assert_eq!(again, first, "the read-only copy is replaced, not refused");
}

#[test]
fn cherry_picking_one_file_takes_only_that_files_change() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["switch", "-c", "dev"]).unwrap();
    f.write_file("file0.txt", "from dev\n").unwrap();
    f.write_file("other.txt", "also dev\n").unwrap();
    f.git(&["add", "--all"]).unwrap();
    f.git_at(5, &["commit", "-m", "dev work"]).unwrap();
    let dev = f.oid("HEAD").unwrap();
    f.git(&["switch", "main"]).unwrap();
    let repo = open(&f);

    repo.apply_commit_file(&dev, "file0.txt", None, false)
        .unwrap();

    assert_eq!(read(&f, "file0.txt"), "from dev\n");
    assert!(!f.path().join("other.txt").exists());
}

#[test]
fn reverting_one_file_undoes_only_that_files_change() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("file0.txt", "second\n").unwrap();
    f.write_file("other.txt", "kept\n").unwrap();
    f.git(&["add", "--all"]).unwrap();
    f.git_at(5, &["commit", "-m", "both"]).unwrap();
    let repo = open(&f);

    repo.apply_commit_file("HEAD", "file0.txt", None, true)
        .unwrap();

    assert_eq!(read(&f, "file0.txt"), "content 0\n");
    assert_eq!(read(&f, "other.txt"), "kept\n");
}

#[test]
fn a_root_commit_can_be_taken_apart_too() {
    let f = test_fixtures::linear(1).unwrap();
    let root = f.oid("HEAD").unwrap();
    let repo = open(&f);

    repo.apply_commit_file(&root, "file0.txt", None, true)
        .unwrap();

    assert!(!f.path().join("file0.txt").exists());
}

#[test]
fn a_file_the_commit_did_not_change_has_nothing_to_apply() {
    let f = test_fixtures::linear(2).unwrap();
    let err = open(&f)
        .apply_commit_file("HEAD", "file0.txt", None, false)
        .unwrap_err();
    assert!(matches!(err, GitError::InvalidState(_)), "{err:?}");
}

#[test]
fn only_the_paths_still_on_disk_are_reported_present() {
    let f = test_fixtures::linear(2).unwrap();
    std::fs::remove_file(f.path().join("file1.txt")).unwrap();
    let present = open(&f).present_on_disk(&paths(&["file0.txt", "file1.txt", "gone.txt"]));
    assert_eq!(present, ["file0.txt"]);
}

// The patch came from porcelain `git diff`, which follows the user's diff.noprefix and
// color.ui: without `a/` and `b/`, or in colour, `git apply` could not read it.
#[test]
fn cherry_picking_one_file_ignores_how_the_user_likes_diffs_shown() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["switch", "-c", "dev"]).unwrap();
    std::fs::create_dir_all(f.path().join("src")).unwrap();
    f.write_file("src/a.txt", "from dev\n").unwrap();
    f.git(&["add", "--all"]).unwrap();
    f.git_at(5, &["commit", "-m", "dev work"]).unwrap();
    let dev = f.oid("HEAD").unwrap();
    f.git(&["switch", "main"]).unwrap();
    f.git(&["config", "diff.noprefix", "true"]).unwrap();
    f.git(&["config", "color.ui", "always"]).unwrap();

    open(&f)
        .apply_commit_file(&dev, "src/a.txt", None, false)
        .unwrap();

    assert_eq!(read(&f, "src/a.txt"), "from dev\n");
}

// The Index Editor saved its working-tree side through the link, into the file it points
// at, which can be outside the repository.
#[test]
fn the_working_tree_side_of_a_symlink_is_not_written_through_it() {
    let f = test_fixtures::linear(1).unwrap();
    let at = f.path().join("link");
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink("file0.txt", &at);
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_file("file0.txt", &at);
    if made.is_err() {
        return;
    }
    let repo = open(&f);

    let written = repo.write_worktree_text("link", "typed in\n");

    assert!(written.is_err(), "{written:?}");
    assert_eq!(read(&f, "file0.txt"), "content 0\n");
}
