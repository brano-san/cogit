#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::discover::{ScanOptions, scan, scan_until};
use std::fs;
use std::path::Path;

fn repo_at(root: &Path, relative: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.join(".git")).unwrap();
}

fn collect(root: &Path, options: &ScanOptions) -> Vec<String> {
    let mut found = Vec::new();
    scan(root, options, |entry| found.push(entry));
    let mut paths: Vec<String> = found
        .iter()
        .map(|entry| {
            entry
                .path
                .strip_prefix(root)
                .unwrap_or(&entry.path)
                .display()
                .to_string()
                .replace('\\', "/")
        })
        .collect();
    paths.sort();
    paths
}

#[test]
fn a_repository_directly_inside_the_folder_is_found() {
    let dir = tempfile::tempdir().unwrap();
    repo_at(dir.path(), "alpha");
    assert_eq!(collect(dir.path(), &ScanOptions::default()), ["alpha"]);
}

#[test]
fn the_scanned_folder_itself_counts_when_it_is_a_repository() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".git")).unwrap();
    assert_eq!(collect(dir.path(), &ScanOptions::default()), [""]);
}

#[test]
fn a_nested_repository_is_found() {
    let dir = tempfile::tempdir().unwrap();
    repo_at(dir.path(), "work/team/beta");
    assert_eq!(
        collect(dir.path(), &ScanOptions::default()),
        ["work/team/beta"]
    );
}

#[test]
fn the_walk_stops_at_a_repository_it_found() {
    let dir = tempfile::tempdir().unwrap();
    repo_at(dir.path(), "outer");
    repo_at(dir.path(), "outer/vendor/inner");
    assert_eq!(collect(dir.path(), &ScanOptions::default()), ["outer"]);
}

#[test]
fn heavy_directories_are_never_entered() {
    let dir = tempfile::tempdir().unwrap();
    repo_at(dir.path(), "node_modules/left-pad");
    repo_at(dir.path(), "target/debug/build/thing");
    repo_at(dir.path(), "keep");
    assert_eq!(collect(dir.path(), &ScanOptions::default()), ["keep"]);
}

#[test]
fn the_depth_limit_is_respected() {
    let dir = tempfile::tempdir().unwrap();
    repo_at(dir.path(), "a/b/c/deep");
    let shallow = ScanOptions { max_depth: 2 };
    assert!(collect(dir.path(), &shallow).is_empty());

    let deeper = ScanOptions { max_depth: 4 };
    assert_eq!(collect(dir.path(), &deeper), ["a/b/c/deep"]);
}

#[test]
fn a_bare_repository_is_recognised() {
    let dir = tempfile::tempdir().unwrap();
    let bare = dir.path().join("mirror.git");
    fs::create_dir_all(bare.join("objects")).unwrap();
    fs::create_dir_all(bare.join("refs")).unwrap();
    fs::write(bare.join("HEAD"), "ref: refs/heads/main\n").unwrap();

    let mut found = Vec::new();
    scan(dir.path(), &ScanOptions::default(), |entry| {
        found.push(entry)
    });
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].bare);
    assert_eq!(found[0].name, "mirror.git");
}

#[test]
fn a_worktree_whose_git_is_a_file_is_found() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("linked")).unwrap();
    fs::write(
        dir.path().join("linked/.git"),
        "gitdir: ../real/.git/worktrees/x\n",
    )
    .unwrap();
    assert_eq!(collect(dir.path(), &ScanOptions::default()), ["linked"]);
}

#[test]
fn the_name_is_the_folder_the_user_would_recognise() {
    let dir = tempfile::tempdir().unwrap();
    repo_at(dir.path(), "projects/cogit");

    let mut found = Vec::new();
    scan(dir.path(), &ScanOptions::default(), |entry| {
        found.push(entry)
    });
    assert_eq!(found[0].name, "cogit");
    assert!(!found[0].bare);
}

#[test]
fn a_missing_folder_yields_nothing_rather_than_failing() {
    let dir = tempfile::tempdir().unwrap();
    let gone = dir.path().join("not-here");
    assert!(collect(&gone, &ScanOptions::default()).is_empty());
}

#[test]
fn every_repository_in_a_broad_tree_is_reported_exactly_once() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..40 {
        repo_at(dir.path(), &format!("group{}/repo{i}", i % 5));
    }
    let paths = collect(dir.path(), &ScanOptions::default());
    assert_eq!(paths.len(), 40);

    let mut unique = paths.clone();
    unique.dedup();
    assert_eq!(unique.len(), 40);
}

#[test]
fn a_broad_tree_is_scanned_inside_the_budget() {
    let dir = tempfile::tempdir().unwrap();
    for group in 0..20 {
        for leaf in 0..50 {
            let deep = format!("group{group}/sub{}/project{leaf}", leaf % 7);
            fs::create_dir_all(dir.path().join(&deep).join("src")).unwrap();
            if leaf % 5 == 0 {
                repo_at(dir.path(), &deep);
            }
        }
    }

    let started = std::time::Instant::now();
    let mut count = 0;
    scan(dir.path(), &ScanOptions::default(), |_| count += 1);
    let elapsed = started.elapsed();

    assert_eq!(count, 200);
    assert!(
        elapsed < std::time::Duration::from_millis(1500),
        "scanning 1000 folders took {elapsed:?}"
    );
}

#[test]
fn a_directory_the_gitignore_names_is_not_walked() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".gitignore"), "vendor/\n").unwrap();
    repo_at(dir.path(), "vendor/thirdparty");
    repo_at(dir.path(), "mine");

    assert_eq!(collect(dir.path(), &ScanOptions::default()), ["mine"]);
}

#[test]
fn an_ignore_rule_reaches_a_folder_further_down() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".gitignore"), "build\n").unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    repo_at(dir.path(), "src/build/generated");
    repo_at(dir.path(), "src/real");

    assert_eq!(collect(dir.path(), &ScanOptions::default()), ["src/real"]);
}

#[test]
fn the_skip_list_still_applies_where_no_gitignore_says_so() {
    let dir = tempfile::tempdir().unwrap();
    repo_at(dir.path(), "node_modules/some-package");
    repo_at(dir.path(), "mine");

    assert_eq!(collect(dir.path(), &ScanOptions::default()), ["mine"]);
}

#[test]
fn a_repository_in_a_hidden_folder_is_still_found() {
    // Hidden is not the same as uninteresting: dotfile repositories live in `.config`.
    let dir = tempfile::tempdir().unwrap();
    repo_at(dir.path(), ".config/dotfiles");

    assert_eq!(
        collect(dir.path(), &ScanOptions::default()),
        [".config/dotfiles"]
    );
}

#[test]
fn an_unignored_rule_wins_over_the_ignore_above_it() {
    let dir = tempfile::tempdir().unwrap();
    // `work/` then `!work/keep` would not work in git either: nothing under an excluded
    // directory can be re-included, because git never looks inside it.
    fs::write(dir.path().join(".gitignore"), "work/*\n!work/keep\n").unwrap();
    repo_at(dir.path(), "work/keep");
    repo_at(dir.path(), "work/drop");

    assert_eq!(collect(dir.path(), &ScanOptions::default()), ["work/keep"]);
}

#[test]
fn a_folder_that_is_not_there_is_scanned_without_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    assert!(collect(&dir.path().join("nowhere"), &ScanOptions::default()).is_empty());
}

#[test]
fn a_folder_the_walk_cannot_read_costs_only_that_folder() {
    let dir = tempfile::tempdir().unwrap();
    repo_at(dir.path(), "reachable");
    // A file where a directory is expected: the walk must step over it, not stop.
    fs::write(dir.path().join("not-a-directory"), "").unwrap();

    assert_eq!(collect(dir.path(), &ScanOptions::default()), ["reachable"]);
}

// Closing the scan dialog only muted the results: the walk went on to the end on the rayon
// pool the diffs and the graph share.
#[test]
fn a_scan_told_to_stop_reports_nothing_more() {
    let dir = tempfile::tempdir().unwrap();
    for n in 0..200 {
        repo_at(dir.path(), &format!("group{}/project{n}", n % 10));
    }

    let mut reported = 0;
    scan_until(dir.path(), &ScanOptions::default(), |_| {
        reported += 1;
        false
    });

    assert_eq!(reported, 1);
}
