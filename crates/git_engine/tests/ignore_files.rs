#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{IgnoreDraft, IgnoreHit, IgnoreScope, RepoHandle};

fn repo_with_global(f: &test_fixtures::Fixture) -> (RepoHandle, std::path::PathBuf) {
    let global = f.path().parent().unwrap().join(format!(
        "{}-global-ignore",
        f.path().file_name().unwrap().to_string_lossy()
    ));
    std::fs::write(&global, "*.tmp\n").unwrap();
    f.git(&["config", "core.excludesFile", &global.to_string_lossy()])
        .unwrap();
    (RepoHandle::open(f.path()).unwrap(), global)
}

fn draft(scope: IgnoreScope, dir: &str, text: &str) -> IgnoreDraft {
    IgnoreDraft {
        scope,
        dir: dir.to_owned(),
        text: text.to_owned(),
    }
}

fn hit<'a>(hits: &'a [IgnoreHit], path: &str) -> Option<&'a IgnoreHit> {
    hits.iter().find(|hit| hit.path == path)
}

#[test]
fn the_three_files_of_a_folder_are_listed() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file(".gitignore", "target/\n").unwrap();
    let (repo, global) = repo_with_global(&f);

    let files = repo.ignore_files("").unwrap();
    let scopes: Vec<IgnoreScope> = files.iter().map(|file| file.scope).collect();
    assert_eq!(
        scopes,
        [
            IgnoreScope::Folder,
            IgnoreScope::Exclude,
            IgnoreScope::Global
        ]
    );
    assert_eq!(files[0].text, "target/\n");
    assert!(files[0].exists);
    assert!(files[1].path.ends_with("info/exclude"));
    assert_eq!(files[2].text, "*.tmp\n");
    assert_eq!(
        std::path::Path::new(&files[2].path).file_name(),
        global.file_name()
    );

    let nested = repo.ignore_files("docs").unwrap();
    assert!(!nested[0].exists);
    assert!(nested[0].path.ends_with("docs/.gitignore"));
}

#[test]
fn the_preview_follows_the_draft_not_the_disk() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("a.log", "").unwrap();
    f.write_file("keep.log", "").unwrap();
    f.write_file("build/out.bin", "").unwrap();
    let (repo, _) = repo_with_global(&f);

    let preview = repo
        .ignore_preview(&[draft(IgnoreScope::Folder, "", "*.log\nbuild/\n!keep.log\n")])
        .unwrap();

    let log = hit(&preview.hits, "a.log").unwrap();
    assert_eq!(
        (log.scope, log.line, log.pattern.as_str()),
        (IgnoreScope::Folder, 1, "*.log")
    );
    assert_eq!(hit(&preview.hits, "build/").unwrap().line, 2);
    assert!(hit(&preview.hits, "keep.log").is_none());
    assert!(!f.path().join(".gitignore").exists());
}

#[test]
fn the_exclude_file_and_the_global_one_take_part() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("x.tmp", "").unwrap();
    f.write_file("local.txt", "").unwrap();
    let (repo, _) = repo_with_global(&f);

    let preview = repo
        .ignore_preview(&[draft(IgnoreScope::Exclude, "", "local.txt\n")])
        .unwrap();

    assert_eq!(
        hit(&preview.hits, "x.tmp").unwrap().scope,
        IgnoreScope::Global
    );
    assert_eq!(
        hit(&preview.hits, "local.txt").unwrap().scope,
        IgnoreScope::Exclude
    );
}

#[test]
fn a_tracked_file_a_rule_matches_is_marked_tracked() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("app.log", "kept\n").unwrap();
    f.git(&["add", "app.log"]).unwrap();
    f.commit_staged(2, "track a log").unwrap();
    let (repo, _) = repo_with_global(&f);

    let preview = repo
        .ignore_preview(&[draft(IgnoreScope::Folder, "", "*.log\n")])
        .unwrap();

    assert!(hit(&preview.hits, "app.log").unwrap().tracked);
}

#[test]
fn a_deeper_gitignore_on_disk_wins_over_the_draft() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("sub/.gitignore", "!*.log\n").unwrap();
    f.git(&["add", "sub/.gitignore"]).unwrap();
    f.commit_staged(2, "nested rules").unwrap();
    f.write_file("sub/b.log", "").unwrap();
    f.write_file("c.log", "").unwrap();
    let (repo, _) = repo_with_global(&f);

    let preview = repo
        .ignore_preview(&[draft(IgnoreScope::Folder, "", "*.log\n")])
        .unwrap();

    assert!(hit(&preview.hits, "c.log").is_some());
    assert!(hit(&preview.hits, "sub/b.log").is_none());
}

#[test]
fn saving_keeps_the_line_endings_the_file_had() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file(".gitignore", "a\r\nb\r\n").unwrap();
    let (repo, _) = repo_with_global(&f);

    repo.save_ignore_file(&draft(IgnoreScope::Folder, "", "a\nb\nc\n"))
        .unwrap();
    assert_eq!(
        std::fs::read(f.path().join(".gitignore")).unwrap(),
        b"a\r\nb\r\nc\r\n"
    );

    repo.save_ignore_file(&draft(IgnoreScope::Folder, "docs", "*.pdf"))
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(f.path().join("docs/.gitignore")).unwrap(),
        "*.pdf\n"
    );
}

#[test]
fn files_under_an_ignored_folder_are_ignored_too() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("build/.gitkeep", "").unwrap();
    f.write_file("build/x.txt", "").unwrap();
    f.git(&["add", "build"]).unwrap();
    f.commit_staged(2, "track build").unwrap();
    f.write_file("build/a.o", "").unwrap();
    let (repo, _) = repo_with_global(&f);

    let preview = repo
        .ignore_preview(&[draft(IgnoreScope::Folder, "", "build/\n!build/a.o\n")])
        .unwrap();

    let untracked = hit(&preview.hits, "build/a.o").unwrap();
    assert_eq!((untracked.line, untracked.tracked), (1, false));
    assert!(hit(&preview.hits, "build/x.txt").unwrap().tracked);
}

#[test]
fn an_untracked_gitignore_takes_part() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("README.md", "").unwrap();
    f.git(&["add", "README.md"]).unwrap();
    f.commit_staged(2, "readme").unwrap();
    f.write_file(".gitignore", "*.bak\n").unwrap();
    f.write_file("old.bak", "").unwrap();
    let (repo, _) = repo_with_global(&f);

    let preview = repo
        .ignore_preview(&[draft(IgnoreScope::Exclude, "", "")])
        .unwrap();

    assert_eq!(hit(&preview.hits, "old.bak").unwrap().line, 1);
}

#[test]
fn a_bare_repository_has_no_ignore_files_to_edit() {
    let f = test_fixtures::bare().unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    assert!(repo.ignore_files("").is_err());
    assert!(repo.ignore_preview(&[]).is_err());
    assert!(
        repo.save_ignore_file(&draft(IgnoreScope::Folder, "", "x\n"))
            .is_err()
    );
    assert!(!f.path().join(".gitignore").exists());
}
