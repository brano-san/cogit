// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{FileStatus, RepoHandle};

fn open(fixture: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(fixture.path()).unwrap()
}

fn paths(files: &[git_engine::FileEntry]) -> Vec<&str> {
    files.iter().map(|f| f.path.as_str()).collect()
}

#[test]
fn commit_details_carry_the_summary_and_both_signatures() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);
    let head = f.oid("HEAD").unwrap();

    let details = repo.commit_details(&head).unwrap();

    assert_eq!(details.oid, head);
    assert_eq!(details.summary, "commit 0");
    assert!(details.body.is_empty());
    assert_eq!(details.author.name, details.committer.name);
    assert!(!details.author.email.is_empty());
    assert!(details.parents.is_empty());
}

#[test]
fn a_multi_line_message_splits_into_summary_and_body() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("more.txt"), "more\n").unwrap();
    f.git(&["add", "--", "more.txt"]).unwrap();
    f.commit_staged(1, "subject line\n\nbody first\nbody second\n")
        .unwrap();

    let repo = open(&f);
    let details = repo.commit_details(&f.oid("HEAD").unwrap()).unwrap();

    assert_eq!(details.summary, "subject line");
    assert_eq!(details.body, "body first\nbody second");
}

#[test]
fn a_root_commit_reports_every_file_as_added() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    let files = repo.commit_files(&f.oid("HEAD").unwrap()).unwrap();

    assert_eq!(paths(&files), ["file0.txt"]);
    assert_eq!(files[0].status, FileStatus::Added);
}

#[test]
fn a_later_commit_reports_only_what_it_changed() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = open(&f);

    let files = repo.commit_files(&f.oid("HEAD").unwrap()).unwrap();

    assert_eq!(paths(&files), ["file2.txt"]);
}

#[test]
fn a_modification_is_reported_as_modified() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::write(f.path().join("file0.txt"), "changed\n").unwrap();
    f.git(&["add", "--", "file0.txt"]).unwrap();
    f.commit_staged(1, "edit file0").unwrap();

    let repo = open(&f);
    let files = repo.commit_files(&f.oid("HEAD").unwrap()).unwrap();

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].status, FileStatus::Modified);
}

#[test]
fn a_deletion_is_reported_as_deleted() {
    let f = test_fixtures::linear(2).unwrap();
    f.git(&["rm", "--", "file0.txt"]).unwrap();
    f.commit_staged(2, "drop file0").unwrap();

    let repo = open(&f);
    let files = repo.commit_files(&f.oid("HEAD").unwrap()).unwrap();

    assert_eq!(paths(&files), ["file0.txt"]);
    assert_eq!(files[0].status, FileStatus::Deleted);
}

#[test]
fn a_rename_keeps_the_path_it_came_from() {
    let f = test_fixtures::renames().unwrap();
    let repo = open(&f);

    let files = repo.commit_files(&f.oid("HEAD").unwrap()).unwrap();

    assert_eq!(files.len(), 1, "a rename is one entry, not a pair");
    assert_eq!(files[0].status, FileStatus::Renamed);
    assert_eq!(files[0].path, "new-name.txt");
    assert_eq!(files[0].old_path.as_deref(), Some("old-name.txt"));
}

#[test]
fn a_mode_change_is_reported_as_modified() {
    let f = test_fixtures::filemode_change().unwrap();
    let repo = open(&f);

    let files = repo.commit_files(&f.oid("HEAD").unwrap()).unwrap();

    assert_eq!(paths(&files), ["script.sh"]);
    assert_eq!(files[0].status, FileStatus::Modified);
}

#[test]
fn a_merge_is_diffed_against_its_first_parent() {
    let f = test_fixtures::diamond().unwrap();
    let repo = open(&f);
    let merge = f.oid("HEAD").unwrap();

    let details = repo.commit_details(&merge).unwrap();
    assert_eq!(details.parents.len(), 2);

    let files = repo.commit_files(&merge).unwrap();
    assert_eq!(
        paths(&files),
        ["dev.txt"],
        "against the first parent only the side branch is new"
    );
}

#[test]
fn nested_paths_cross_the_boundary_with_forward_slashes() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::create_dir_all(f.path().join("src/deep")).unwrap();
    std::fs::write(f.path().join("src/deep/mod.rs"), "fn main() {}\n").unwrap();
    f.git(&["add", "--", "src/deep/mod.rs"]).unwrap();
    f.commit_staged(1, "add a nested file").unwrap();

    let repo = open(&f);
    let files = repo.commit_files(&f.oid("HEAD").unwrap()).unwrap();

    assert_eq!(paths(&files), ["src/deep/mod.rs"]);
}

#[test]
fn unicode_paths_survive_the_round_trip() {
    let f = test_fixtures::unicode_paths().unwrap();
    let repo = open(&f);

    let files = repo.commit_files(&f.oid("HEAD").unwrap()).unwrap();

    assert!(
        files.iter().all(|e| !e.path.contains('\\')),
        "octal escaping or backslashes leaked into {:?}",
        paths(&files)
    );
}

#[test]
fn an_unknown_oid_is_a_typed_error() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = open(&f);

    assert!(repo.commit_details("0".repeat(40).as_str()).is_err());
    assert!(repo.commit_files("not-a-hex-oid").is_err());
}

#[test]
fn an_empty_repository_answers_without_panicking() {
    let f = test_fixtures::empty().unwrap();
    let repo = open(&f);

    assert!(repo.commit_details("HEAD").is_err());
}
