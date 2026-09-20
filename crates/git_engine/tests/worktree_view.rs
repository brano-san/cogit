#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The Files panel offers SmartGit's view toggles (T6.9). Everything a toggle can show has
//! to come out of `worktree_files_with`, because the panel cannot invent it.

use git_engine::{FileStatus, RepoHandle, WorktreeView};

fn prepared() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(3).unwrap();
    std::fs::write(f.path().join(".gitignore"), "build/\n*.tmp\n").unwrap();
    std::fs::create_dir_all(f.path().join("build")).unwrap();
    std::fs::write(f.path().join("build/out.o"), "binary\n").unwrap();
    std::fs::write(f.path().join("scratch.tmp"), "junk\n").unwrap();
    f.git(&["add", ".gitignore"]).unwrap();
    f.git(&["commit", "-m", "ignore build output"]).unwrap();
    f
}

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn paths(entries: &[git_engine::FileEntry], status: FileStatus) -> Vec<String> {
    let mut found: Vec<String> = entries
        .iter()
        .filter(|entry| entry.status == status)
        .map(|entry| entry.path.clone())
        .collect();
    found.sort();
    found
}

#[test]
fn the_default_view_shows_no_unchanged_file() {
    let f = prepared();
    let files = open(&f)
        .worktree_files_with(WorktreeView::default())
        .unwrap();
    assert!(paths(&files.unstaged, FileStatus::Unchanged).is_empty());
}

#[test]
fn asking_for_unchanged_lists_every_quiet_tracked_file() {
    let f = prepared();
    let view = WorktreeView {
        unchanged: true,
        ..WorktreeView::default()
    };
    let files = open(&f).worktree_files_with(view).unwrap();
    let quiet = paths(&files.unstaged, FileStatus::Unchanged);
    assert!(quiet.contains(&"file0.txt".to_owned()), "{quiet:?}");
    assert!(quiet.contains(&".gitignore".to_owned()), "{quiet:?}");
}

#[test]
fn an_edited_file_is_never_reported_as_unchanged() {
    let f = prepared();
    std::fs::write(f.path().join("file0.txt"), "edited\n").unwrap();
    let view = WorktreeView {
        unchanged: true,
        ..WorktreeView::default()
    };
    let files = open(&f).worktree_files_with(view).unwrap();

    assert!(!paths(&files.unstaged, FileStatus::Unchanged).contains(&"file0.txt".to_owned()));
    assert!(paths(&files.unstaged, FileStatus::Modified).contains(&"file0.txt".to_owned()));
}

#[test]
fn the_default_view_hides_ignored_files() {
    let f = prepared();
    let files = open(&f)
        .worktree_files_with(WorktreeView::default())
        .unwrap();
    assert!(paths(&files.unstaged, FileStatus::Ignored).is_empty());
}

#[test]
fn asking_for_ignored_lists_what_gitignore_hides() {
    let f = prepared();
    let view = WorktreeView {
        ignored: true,
        ..WorktreeView::default()
    };
    let files = open(&f).worktree_files_with(view).unwrap();
    let hidden = paths(&files.unstaged, FileStatus::Ignored);
    assert!(
        hidden.iter().any(|path| path.starts_with("build")),
        "{hidden:?}"
    );
    assert!(hidden.contains(&"scratch.tmp".to_owned()), "{hidden:?}");
}

#[test]
fn an_assume_unchanged_file_appears_only_when_asked_for() {
    let f = prepared();
    f.git(&["update-index", "--assume-unchanged", "file1.txt"])
        .unwrap();

    let plain = open(&f)
        .worktree_files_with(WorktreeView::default())
        .unwrap();
    assert!(paths(&plain.unstaged, FileStatus::AssumeUnchanged).is_empty());

    let view = WorktreeView {
        assume_unchanged: true,
        ..WorktreeView::default()
    };
    let asked = open(&f).worktree_files_with(view).unwrap();
    assert_eq!(
        paths(&asked.unstaged, FileStatus::AssumeUnchanged),
        ["file1.txt"]
    );
}

#[test]
fn a_skip_worktree_file_appears_only_when_asked_for() {
    let f = prepared();
    f.git(&["update-index", "--skip-worktree", "file2.txt"])
        .unwrap();

    let plain = open(&f)
        .worktree_files_with(WorktreeView::default())
        .unwrap();
    assert!(paths(&plain.unstaged, FileStatus::Skipped).is_empty());

    let view = WorktreeView {
        skipped: true,
        ..WorktreeView::default()
    };
    let asked = open(&f).worktree_files_with(view).unwrap();
    assert_eq!(paths(&asked.unstaged, FileStatus::Skipped), ["file2.txt"]);
}

#[test]
fn a_flagged_file_is_not_also_listed_as_unchanged() {
    let f = prepared();
    f.git(&["update-index", "--skip-worktree", "file2.txt"])
        .unwrap();
    let view = WorktreeView {
        unchanged: true,
        skipped: true,
        ..WorktreeView::default()
    };
    let files = open(&f).worktree_files_with(view).unwrap();
    assert!(!paths(&files.unstaged, FileStatus::Unchanged).contains(&"file2.txt".to_owned()));
}

#[test]
fn the_extra_views_leave_the_ordinary_lists_alone() {
    let f = prepared();
    std::fs::write(f.path().join("file0.txt"), "edited\n").unwrap();
    f.git(&["add", "file0.txt"]).unwrap();
    std::fs::write(f.path().join("file1.txt"), "also edited\n").unwrap();

    let plain = open(&f)
        .worktree_files_with(WorktreeView::default())
        .unwrap();
    let everything = open(&f)
        .worktree_files_with(WorktreeView {
            unchanged: true,
            ignored: true,
            assume_unchanged: true,
            skipped: true,
        })
        .unwrap();

    assert_eq!(plain.staged, everything.staged);
    assert_eq!(
        paths(&plain.unstaged, FileStatus::Modified),
        paths(&everything.unstaged, FileStatus::Modified)
    );
}
