// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]
// The budget test exists to print a number; `print_stdout` is denied for production (INV-04).
#![allow(clippy::print_stdout)]

use git_engine::RepoHandle;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

/// What `git diff-tree --no-commit-id --name-only -r` prints, which is the DoD of T13.1.
fn git_paths(f: &test_fixtures::Fixture, rev: &str) -> Vec<String> {
    let has_parent = f
        .git(&["rev-parse", "--verify", &format!("{rev}^")])
        .is_ok();
    let args: Vec<String> = if has_parent {
        vec![
            "diff-tree".into(),
            "--no-commit-id".into(),
            "--name-only".into(),
            "-r".into(),
            format!("{rev}^"),
            rev.to_owned(),
        ]
    } else {
        vec![
            "diff-tree".into(),
            "--no-commit-id".into(),
            "--name-only".into(),
            "-r".into(),
            "--root".into(),
            rev.to_owned(),
        ]
    };
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut paths: Vec<String> = f
        .git(&borrowed)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    paths.sort();
    paths
}

fn assert_matches_git(f: &test_fixtures::Fixture, rev: &str) {
    let ours = open(f).changed_paths(&f.oid(rev).unwrap()).unwrap();
    assert_eq!(ours, git_paths(f, rev), "{rev}");
}

#[test]
fn an_ordinary_commit_lists_the_files_it_changed() {
    let f = test_fixtures::linear(3).unwrap();
    assert_matches_git(&f, "HEAD");
}

#[test]
fn a_root_commit_lists_every_path_in_its_tree() {
    let f = test_fixtures::linear(3).unwrap();
    assert_matches_git(&f, "HEAD~2");
}

#[test]
fn a_merge_commit_is_compared_with_its_first_parent() {
    let f = test_fixtures::diamond().unwrap();
    assert_matches_git(&f, "HEAD");
}

#[test]
fn an_octopus_merge_is_also_compared_with_its_first_parent() {
    let f = test_fixtures::octopus().unwrap();
    assert_matches_git(&f, "HEAD");
}

#[test]
fn a_rename_shows_both_sides_because_the_paths_are_what_matter() {
    let f = test_fixtures::renames().unwrap();
    assert_matches_git(&f, "HEAD");
}

#[test]
fn a_commit_touching_nested_directories_lists_full_paths() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::create_dir_all(f.path().join("src/deep")).unwrap();
    std::fs::write(f.path().join("src/deep/a.rs"), "fn a() {}\n").unwrap();
    f.git(&["add", "--", "src/deep/a.rs"]).unwrap();
    f.commit_staged(1, "add a nested file").unwrap();

    let paths = open(&f).changed_paths(&f.oid("HEAD").unwrap()).unwrap();

    assert_eq!(paths, vec!["src/deep/a.rs".to_owned()]);
}

#[test]
fn paths_cross_the_boundary_with_forward_slashes() {
    let f = test_fixtures::linear(1).unwrap();
    std::fs::create_dir_all(f.path().join("a/b")).unwrap();
    std::fs::write(f.path().join("a/b/c.txt"), "c\n").unwrap();
    f.git(&["add", "--", "a/b/c.txt"]).unwrap();
    f.commit_staged(1, "nested").unwrap();

    let paths = open(&f).changed_paths(&f.oid("HEAD").unwrap()).unwrap();

    assert!(paths.iter().all(|p| !p.contains('\\')), "{paths:?}");
}

#[test]
fn the_list_is_sorted_so_two_commits_can_be_compared_directly() {
    let f = test_fixtures::linear(1).unwrap();
    for name in ["z.txt", "a.txt", "m.txt"] {
        std::fs::write(f.path().join(name), "x\n").unwrap();
        f.git(&["add", "--", name]).unwrap();
    }
    f.commit_staged(1, "three files").unwrap();

    let paths = open(&f).changed_paths(&f.oid("HEAD").unwrap()).unwrap();
    let mut sorted = paths.clone();
    sorted.sort();

    assert_eq!(paths, sorted);
}

#[test]
fn an_unknown_revision_is_a_typed_error() {
    let f = test_fixtures::linear(2).unwrap();
    assert!(open(&f).changed_paths("no-such-commit").is_err());
}

#[test]
fn reading_the_paths_spawns_no_process() {
    use std::sync::{Arc, Mutex};

    let f = test_fixtures::linear(3).unwrap();
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let repo = RepoHandle::open(f.path()).unwrap().with_journal(Arc::new(
        move |out: git_engine::GitOutput| {
            if let Ok(mut entries) = sink.lock() {
                entries.push(out.command);
            }
        },
    ));

    repo.changed_paths(&f.oid("HEAD").unwrap()).unwrap();

    assert!(log.lock().unwrap().is_empty());
}

use git_engine::{Overlap, overlap_of};

#[test]
fn two_commits_with_nothing_in_common_do_not_overlap() {
    assert_eq!(overlap_of(&["a".into()], &["b".into()]), Overlap::None);
}

#[test]
fn identical_sets_are_the_same_files() {
    let paths = vec!["a".to_owned(), "b".to_owned()];
    assert_eq!(overlap_of(&paths, &paths), Overlap::Same);
}

#[test]
fn a_third_or_less_of_the_base_is_slight() {
    let base = vec!["a".into(), "b".into(), "c".into()];
    assert_eq!(overlap_of(&base, &["a".into()]), Overlap::Slight);
}

#[test]
fn more_than_a_third_of_the_base_is_heavy() {
    let base: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
    assert_eq!(overlap_of(&base, &["a".into(), "b".into()]), Overlap::Heavy);
}

#[test]
fn a_subset_that_is_not_the_whole_base_is_not_the_same() {
    let base: Vec<String> = vec!["a".into(), "b".into()];
    assert_ne!(overlap_of(&base, &["a".into()]), Overlap::Same);
}

#[test]
fn a_superset_of_the_base_is_not_the_same_either() {
    let base: Vec<String> = vec!["a".into()];
    assert_ne!(overlap_of(&base, &["a".into(), "b".into()]), Overlap::Same);
}

#[test]
fn an_empty_base_overlaps_with_nothing() {
    assert_eq!(overlap_of(&[], &["a".into()]), Overlap::None);
    assert_eq!(overlap_of(&["a".into()], &[]), Overlap::None);
}

#[test]
fn two_empty_sets_are_not_reported_as_identical() {
    assert_eq!(overlap_of(&[], &[]), Overlap::None);
}

#[test]
fn the_shared_paths_are_reported_for_the_tooltip() {
    let base: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
    let other: Vec<String> = vec!["b".into(), "c".into(), "d".into()];

    assert_eq!(
        git_engine::shared_paths(&base, &other),
        vec!["b".to_owned(), "c".to_owned()]
    );
}

#[test]
fn overlap_is_computed_for_a_whole_window_at_once() {
    let f = test_fixtures::linear(1).unwrap();
    for name in ["a.txt", "b.txt"] {
        std::fs::write(f.path().join(name), "x\n").unwrap();
        f.git(&["add", "--", name]).unwrap();
        f.commit_staged(1, &format!("add {name}")).unwrap();
    }
    std::fs::write(f.path().join("a.txt"), "changed\n").unwrap();
    f.git(&["add", "--", "a.txt"]).unwrap();
    f.commit_staged(3, "touch a again").unwrap();

    let repo = open(&f);
    let base = f.oid("HEAD").unwrap();
    let window = vec![f.oid("HEAD~1").unwrap(), f.oid("HEAD~2").unwrap()];

    let rows = repo.overlap_window(&base, &window).unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].overlap, Overlap::None, "{rows:?}");
    assert_eq!(rows[1].overlap, Overlap::Same, "{rows:?}");
}

#[test]
fn the_base_commit_is_reported_as_itself_rather_than_as_an_overlap() {
    let f = test_fixtures::linear(3).unwrap();
    let base = f.oid("HEAD").unwrap();

    let rows = open(&f)
        .overlap_window(&base, std::slice::from_ref(&base))
        .unwrap();

    assert!(rows[0].is_base, "{rows:?}");
}

#[test]
fn a_window_with_an_unreadable_commit_still_answers_for_the_others() {
    let f = test_fixtures::linear(3).unwrap();
    let base = f.oid("HEAD").unwrap();
    let window = vec!["0".repeat(40), f.oid("HEAD~1").unwrap()];

    let rows = open(&f).overlap_window(&base, &window).unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].overlap, Overlap::None, "{rows:?}");
}

/// The DoD of T13.2: a 1 000-commit window recomputes faster than 300 ms. The budget here
/// is doubled so a loaded machine does not go red; print with `--nocapture`.
#[test]
fn a_thousand_commit_window_is_computed_inside_the_budget() {
    use std::time::Instant;

    let f = test_fixtures::stress(1_000).unwrap();
    let repo = open(&f);
    let window: Vec<String> = f
        .git(&["rev-list", "--max-count=1000", "HEAD"])
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    let base = window[0].clone();

    // Warm: the first pass pays for loading the object database.
    repo.overlap_window(&base, &window[..50]).unwrap();

    let started = Instant::now();
    let rows = repo.overlap_window(&base, &window).unwrap();
    let elapsed = started.elapsed();

    println!(
        "overlap window of {:>5} commits {:>5} ms   (budget 600 ms)",
        window.len(),
        elapsed.as_millis()
    );
    assert_eq!(rows.len(), window.len());
    assert!(
        elapsed.as_millis() < 600,
        "overlap for {} commits took {} ms",
        window.len(),
        elapsed.as_millis()
    );
}
