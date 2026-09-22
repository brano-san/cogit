// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{FileMode, RepoHandle};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

#[test]
fn an_ordinary_file_reports_no_mode_change() {
    let f = test_fixtures::linear(2).unwrap();
    let files = open(&f).commit_files("HEAD").unwrap();

    assert_eq!(files[0].mode_change, None);
}

#[test]
fn making_a_file_executable_is_reported_as_a_mode_change() {
    let f = test_fixtures::filemode_change().unwrap();

    let files = open(&f).commit_files("HEAD").unwrap();

    assert_eq!(
        files[0].mode_change,
        Some(FileMode::Executable),
        "{:?}",
        files[0]
    );
}

#[test]
fn dropping_the_executable_bit_is_reported_too() {
    let f = test_fixtures::filemode_change().unwrap();
    f.git(&["update-index", "--chmod=-x", "script.sh"]).unwrap();
    f.commit_staged(2, "make script.sh plain again").unwrap();

    let files = open(&f).commit_files("HEAD").unwrap();

    assert_eq!(files[0].mode_change, Some(FileMode::Plain));
}

#[test]
fn a_rename_carries_how_similar_the_two_sides_are() {
    let f = test_fixtures::renames().unwrap();

    let files = open(&f).commit_files("HEAD").unwrap();

    assert_eq!(files[0].status, git_engine::FileStatus::Renamed);
    assert_eq!(
        files[0].similarity,
        Some(100),
        "an untouched rename is a perfect match: {:?}",
        files[0]
    );
}

#[test]
fn a_rename_with_edits_reports_less_than_a_perfect_match() {
    let f = test_fixtures::linear(1).unwrap();
    let body: String = (0..30).map(|i| format!("line {i}\n")).collect();
    f.write_file("before.txt", &body).unwrap();
    f.git(&["add", "--", "before.txt"]).unwrap();
    f.commit_staged(1, "add before.txt").unwrap();

    f.git(&["mv", "before.txt", "after.txt"]).unwrap();
    f.write_file("after.txt", &body.replace("line 0\n", "changed\n"))
        .unwrap();
    f.git(&["add", "--", "after.txt"]).unwrap();
    f.commit_staged(2, "rename and edit").unwrap();

    let files = open(&f).commit_files("HEAD").unwrap();
    let similarity = files[0].similarity.unwrap();

    assert!(similarity < 100 && similarity > 50, "got {similarity}");
}

#[test]
fn an_added_file_has_no_similarity_to_report() {
    let f = test_fixtures::linear(1).unwrap();
    assert_eq!(open(&f).commit_files("HEAD").unwrap()[0].similarity, None);
}

#[test]
fn the_rename_threshold_can_be_lowered_to_catch_a_heavier_rewrite() {
    let f = test_fixtures::linear(1).unwrap();
    let body: String = (0..20).map(|i| format!("line {i}\n")).collect();
    f.write_file("before.txt", &body).unwrap();
    f.git(&["add", "--", "before.txt"]).unwrap();
    f.commit_staged(1, "add before.txt").unwrap();

    f.git(&["rm", "--", "before.txt"]).unwrap();
    let rewritten: String = (0..20)
        .map(|i| {
            if i < 12 {
                format!("new {i}\n")
            } else {
                format!("line {i}\n")
            }
        })
        .collect();
    f.write_file("after.txt", &rewritten).unwrap();
    f.git(&["add", "--", "after.txt"]).unwrap();
    f.commit_staged(2, "heavy rewrite under a new name")
        .unwrap();
    let repo = open(&f);

    let strict = repo.commit_files_with("HEAD", 90).unwrap();
    let loose = repo.commit_files_with("HEAD", 20).unwrap();

    assert_eq!(strict.len(), 2, "at 90% these are an add and a delete");
    assert_eq!(loose.len(), 1, "at 20% they are one rename");
}

/// Copy detection only looks at files the commit already touched, as `git diff -C` does;
/// searching the whole tree is quadratic and Git makes it opt-in too.
#[test]
fn a_copy_from_a_file_the_commit_also_touched_is_detected() {
    let f = test_fixtures::linear(1).unwrap();
    let body: String = (0..30)
        .map(|i| {
            format!(
                "line {i}
"
            )
        })
        .collect();
    f.write_file("source.txt", &body).unwrap();
    f.git(&["add", "--", "source.txt"]).unwrap();
    f.commit_staged(1, "add source.txt").unwrap();

    f.write_file(
        "source.txt",
        &format!(
            "{body}extra
"
        ),
    )
    .unwrap();
    f.write_file("copy.txt", &body).unwrap();
    f.git(&["add", "--", "source.txt", "copy.txt"]).unwrap();
    f.commit_staged(2, "copy the file and edit the source")
        .unwrap();

    let files = open(&f).commit_files("HEAD").unwrap();
    let copy = files.iter().find(|e| e.path == "copy.txt").unwrap();

    assert_eq!(copy.status, git_engine::FileStatus::Copied, "{copy:?}");
    assert_eq!(copy.old_path.as_deref(), Some("source.txt"));
}

#[test]
fn a_copy_from_an_untouched_file_reads_as_an_addition() {
    let f = test_fixtures::linear(1).unwrap();
    let body: String = (0..30)
        .map(|i| {
            format!(
                "line {i}
"
            )
        })
        .collect();
    f.write_file("source.txt", &body).unwrap();
    f.git(&["add", "--", "source.txt"]).unwrap();
    f.commit_staged(1, "add source.txt").unwrap();

    f.write_file("copy.txt", &body).unwrap();
    f.git(&["add", "--", "copy.txt"]).unwrap();
    f.commit_staged(2, "copy the file").unwrap();

    let files = open(&f).commit_files("HEAD").unwrap();

    assert_eq!(files[0].status, git_engine::FileStatus::Added);
}

/// `kors` with `M` drawn as a file: the working-tree side never read the entry's mode
/// (doc/12-risks.md, R-180).
#[test]
fn a_submodule_moved_in_the_working_tree_is_listed_as_a_submodule() {
    let f = test_fixtures::with_submodule().unwrap();
    f.git(&["-C", "vendor/lib", "checkout", "-q", "HEAD~1"])
        .unwrap();

    let files = open(&f).worktree_files().unwrap();
    let module = files
        .unstaged
        .iter()
        .find(|file| file.path == "vendor/lib")
        .expect("the moved submodule is listed");

    assert_eq!(module.status, git_engine::FileStatus::Modified);
    assert_eq!(module.mode, FileMode::Submodule);
}

#[test]
fn a_staged_new_submodule_is_listed_as_a_submodule() {
    let f = test_fixtures::with_submodule().unwrap();
    f.git(&["rm", "-q", "--cached", "vendor/lib"]).unwrap();
    f.commit_staged(2, "forget the submodule").unwrap();
    f.git(&["add", "vendor/lib"]).unwrap();

    let files = open(&f).worktree_files().unwrap();
    let module = files
        .staged
        .iter()
        .find(|file| file.path == "vendor/lib")
        .expect("the staged submodule is listed");

    assert_eq!(module.status, git_engine::FileStatus::Added);
    assert_eq!(module.mode, FileMode::Submodule);
}

#[cfg(unix)]
#[test]
fn a_changed_symlink_in_the_working_tree_is_listed_as_a_symlink() {
    let f = test_fixtures::linear(1).unwrap();
    std::os::unix::fs::symlink("file0.txt", f.path().join("link")).unwrap();
    f.git(&["add", "link"]).unwrap();
    f.commit_staged(1, "add a link").unwrap();
    std::fs::remove_file(f.path().join("link")).unwrap();
    std::os::unix::fs::symlink("elsewhere", f.path().join("link")).unwrap();

    let files = open(&f).worktree_files().unwrap();
    let link = files
        .unstaged
        .iter()
        .find(|file| file.path == "link")
        .unwrap();

    assert_eq!(link.mode, FileMode::Symlink);
}
