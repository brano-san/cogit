// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::RepoHandle;

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

/// Two branches whose tips both rewrite `shared.txt`, so a rebase is bound to stop.
fn conflicting_rebase() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("shared.txt", "base\n").unwrap();
    f.git(&["add", "--", "shared.txt"]).unwrap();
    f.commit_staged(1, "add shared").unwrap();

    f.git(&["checkout", "-b", "topic"]).unwrap();
    f.write_file("shared.txt", "topic\n").unwrap();
    f.git(&["add", "--", "shared.txt"]).unwrap();
    f.commit_staged(2, "topic edit").unwrap();
    f.write_file("second.txt", "second\n").unwrap();
    f.git(&["add", "--", "second.txt"]).unwrap();
    f.commit_staged(3, "topic second").unwrap();

    f.git(&["checkout", "main"]).unwrap();
    f.write_file("shared.txt", "main\n").unwrap();
    f.git(&["add", "--", "shared.txt"]).unwrap();
    f.commit_staged(4, "main edit").unwrap();

    f.git(&["checkout", "topic"]).unwrap();
    let _ = f.git(&["rebase", "main"]);
    f
}

#[test]
fn a_calm_repository_has_no_operation_in_progress() {
    let f = test_fixtures::linear(3).unwrap();
    assert!(open(&f).rebase_progress().unwrap().is_none());
}

#[test]
fn a_stopped_rebase_reports_the_commit_it_is_applying() {
    let f = conflicting_rebase();

    let progress = open(&f).rebase_progress().unwrap().unwrap();

    assert_eq!(
        progress.applying.as_deref(),
        Some("topic edit"),
        "{progress:?}"
    );
}

#[test]
fn a_stopped_rebase_reports_where_it_is_replaying_onto() {
    let f = conflicting_rebase();

    let progress = open(&f).rebase_progress().unwrap().unwrap();

    assert_eq!(
        progress.onto.as_deref(),
        Some(&*f.oid("main").unwrap()),
        "{progress:?}"
    );
}

#[test]
fn a_stopped_rebase_counts_the_steps_it_has_left() {
    let f = conflicting_rebase();

    let progress = open(&f).rebase_progress().unwrap().unwrap();

    assert_eq!(progress.total, 2, "{progress:?}");
    assert!(progress.done >= 1, "{progress:?}");
}

#[test]
fn the_remaining_steps_are_listed_with_their_subjects() {
    let f = conflicting_rebase();

    let progress = open(&f).rebase_progress().unwrap().unwrap();

    assert_eq!(progress.todo.len(), 1, "{progress:?}");
    assert_eq!(progress.todo[0].summary, "topic second", "{progress:?}");
    assert_eq!(progress.todo[0].action, "pick", "{progress:?}");
}

#[test]
fn an_unreadable_todo_file_costs_the_step_list_but_not_the_banner() {
    let f = conflicting_rebase();
    std::fs::write(
        f.path().join(".git/rebase-merge/git-rebase-todo"),
        "nonsense\n",
    )
    .unwrap();

    let progress = open(&f).rebase_progress().unwrap().unwrap();

    assert!(progress.todo.is_empty(), "{progress:?}");
    assert!(progress.applying.is_some(), "{progress:?}");
}

#[test]
fn reading_the_progress_spawns_no_process() {
    use std::sync::{Arc, Mutex};

    let f = conflicting_rebase();
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let repo = RepoHandle::open(f.path()).unwrap().with_journal(Arc::new(
        move |out: git_engine::GitOutput| {
            if let Ok(mut entries) = sink.lock() {
                entries.push(out.command);
            }
        },
    ));

    repo.rebase_progress().unwrap();

    assert!(log.lock().unwrap().is_empty());
}

/// A `git-rebase-todo` written by a hand or a tool Cogit does not know about. The panel
/// must show what it can and refuse to fall over (M11).
mod damaged_todo {
    use super::*;

    fn paused(body: &str) -> test_fixtures::Fixture {
        let f = test_fixtures::linear(3).unwrap();
        let merge = f.path().join(".git").join("rebase-merge");
        std::fs::create_dir_all(&merge).unwrap();
        std::fs::write(merge.join("git-rebase-todo"), body).unwrap();
        std::fs::write(merge.join("done"), "").unwrap();
        f
    }

    #[test]
    fn a_todo_full_of_nonsense_still_reports_a_rebase() {
        let f = paused("this is not a todo file at all\n\u{0}\u{1}\n");
        let progress = open(&f).rebase_progress().unwrap();
        assert!(progress.is_some());
    }

    #[test]
    fn an_unknown_action_does_not_lose_the_line() {
        let f = paused("teleport abc1234 do something odd\n");
        let progress = open(&f).rebase_progress().unwrap().unwrap();
        assert_eq!(progress.todo.len(), 1, "{:?}", progress.todo);
    }

    #[test]
    fn a_line_with_no_oid_is_skipped_rather_than_guessed_at() {
        let f = paused("pick\n");
        let progress = open(&f).rebase_progress().unwrap().unwrap();
        assert!(progress.todo.is_empty(), "{:?}", progress.todo);
    }

    #[test]
    fn an_empty_todo_means_nothing_is_left_to_do() {
        let f = paused("");
        let progress = open(&f).rebase_progress().unwrap().unwrap();
        assert_eq!(progress.total, progress.done);
    }

    #[test]
    fn a_todo_that_cannot_be_read_at_all_is_not_an_error() {
        let f = test_fixtures::linear(2).unwrap();
        std::fs::create_dir_all(f.path().join(".git").join("rebase-merge")).unwrap();
        assert!(open(&f).rebase_progress().is_ok());
    }
}

// Cogit's own plans put an `exec` after a reword and a `break` after every step of a paused
// rebase. The steps left were counted without them, the steps done with them.
#[test]
fn a_paused_rebase_counts_only_its_commits() {
    let f = test_fixtures::linear(4).unwrap();
    let repo = git_engine::RepoHandle::open(f.path()).unwrap();
    let base = f.oid("HEAD~3").unwrap();
    let plan = repo.rebase_todo(&base).unwrap();
    assert_eq!(plan.len(), 3);
    let _ = repo.interactive_rebase_paused(&base, &plan);

    let progress = repo.rebase_progress().unwrap().unwrap();

    assert_eq!((progress.done, progress.total), (1, 3));
}
