// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{RepoHandle, TodoAction, TodoEntry, render_todo};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

fn entry(oid: &str, action: TodoAction) -> TodoEntry {
    TodoEntry {
        oid: oid.to_owned(),
        action,
        message: None,
    }
}

fn subjects(f: &test_fixtures::Fixture) -> Vec<String> {
    f.git(&["log", "--format=%s"])
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn a_plan_of_picks_renders_one_line_each_in_order() {
    let plan = [
        entry("aaaa", TodoAction::Pick),
        entry("bbbb", TodoAction::Pick),
    ];

    assert_eq!(render_todo(&plan), "pick aaaa\npick bbbb\n");
}

#[test]
fn every_action_has_the_verb_git_expects() {
    let plan = [
        entry("a", TodoAction::Pick),
        entry("b", TodoAction::Reword),
        entry("c", TodoAction::Edit),
        entry("d", TodoAction::Squash),
        entry("e", TodoAction::Fixup),
        entry("f", TodoAction::Drop),
    ];

    let todo = render_todo(&plan);

    assert!(todo.contains("pick a\n"), "{todo}");
    assert!(
        todo.contains("reword b\n") || todo.contains("pick b\n"),
        "{todo}"
    );
    assert!(todo.contains("edit c\n"), "{todo}");
    assert!(todo.contains("squash d\n"), "{todo}");
    assert!(todo.contains("fixup e\n"), "{todo}");
    assert!(todo.contains("drop f\n"), "{todo}");
}

#[test]
fn a_new_message_becomes_an_amend_step_rather_than_an_editor_prompt() {
    let plan = [TodoEntry {
        oid: "aaaa".to_owned(),
        action: TodoAction::Reword,
        message: Some("a better subject".to_owned()),
    }];

    let todo = render_todo(&plan);

    assert!(todo.contains("exec git commit --amend"), "{todo}");
    assert!(todo.contains("a better subject"), "{todo}");
}

#[test]
fn a_quote_in_a_message_cannot_break_out_of_the_exec_line() {
    let plan = [TodoEntry {
        oid: "aaaa".to_owned(),
        action: TodoAction::Reword,
        message: Some("it's here; rm -rf /".to_owned()),
    }];

    let todo = render_todo(&plan);
    let line = todo.lines().find(|l| l.starts_with("exec")).unwrap();

    assert!(line.ends_with('\''), "{line}");
    assert!(
        !line.contains("' "),
        "an unescaped quote ends the argument: {line}"
    );
}

#[test]
fn reordering_two_commits_changes_their_order() {
    let f = test_fixtures::linear(4).unwrap();
    let base = f.oid("HEAD~2").unwrap();
    let last = f.oid("HEAD").unwrap();
    let first = f.oid("HEAD~1").unwrap();

    open(&f)
        .interactive_rebase(
            &base,
            &[
                entry(&last, TodoAction::Pick),
                entry(&first, TodoAction::Pick),
            ],
        )
        .unwrap();

    assert_eq!(subjects(&f)[0], "commit 2", "{:?}", subjects(&f));
}

#[test]
fn squashing_leaves_one_commit_where_there_were_two() {
    let f = test_fixtures::linear(4).unwrap();
    let base = f.oid("HEAD~2").unwrap();
    let before = f.git(&["rev-list", "--count", "HEAD"]).unwrap();

    open(&f)
        .interactive_rebase(
            &base,
            &[
                entry(&f.oid("HEAD~1").unwrap(), TodoAction::Pick),
                entry(&f.oid("HEAD").unwrap(), TodoAction::Squash),
            ],
        )
        .unwrap();

    let after = f.git(&["rev-list", "--count", "HEAD"]).unwrap();
    assert_eq!(
        after.trim().parse::<u32>().unwrap(),
        before.trim().parse::<u32>().unwrap() - 1
    );
}

#[test]
fn dropping_a_commit_removes_it_from_the_history() {
    let f = test_fixtures::linear(4).unwrap();
    let base = f.oid("HEAD~2").unwrap();

    open(&f)
        .interactive_rebase(
            &base,
            &[
                entry(&f.oid("HEAD~1").unwrap(), TodoAction::Drop),
                entry(&f.oid("HEAD").unwrap(), TodoAction::Pick),
            ],
        )
        .unwrap();

    assert!(
        !subjects(&f).contains(&"commit 2".to_owned()),
        "{:?}",
        subjects(&f)
    );
}

#[test]
fn rewording_replaces_the_subject() {
    let f = test_fixtures::linear(3).unwrap();
    let base = f.oid("HEAD~1").unwrap();

    open(&f)
        .interactive_rebase(
            &base,
            &[TodoEntry {
                oid: f.oid("HEAD").unwrap(),
                action: TodoAction::Reword,
                message: Some("a much better subject".to_owned()),
            }],
        )
        .unwrap();

    assert_eq!(subjects(&f)[0], "a much better subject");
}

#[test]
fn an_empty_plan_is_refused() {
    let f = test_fixtures::linear(3).unwrap();
    assert!(
        open(&f)
            .interactive_rebase(&f.oid("HEAD~1").unwrap(), &[])
            .is_err()
    );
}

#[test]
fn a_dirty_working_tree_stops_the_rebase_before_it_starts() {
    let f = test_fixtures::linear(3).unwrap();
    f.write_file("file0.txt", "uncommitted\n").unwrap();

    assert!(
        open(&f)
            .interactive_rebase(
                &f.oid("HEAD~1").unwrap(),
                &[entry(&f.oid("HEAD").unwrap(), TodoAction::Pick)],
            )
            .is_err()
    );
}

#[test]
fn the_default_plan_lists_the_range_oldest_first_as_picks() {
    let f = test_fixtures::linear(4).unwrap();

    let plan = open(&f).rebase_todo(&f.oid("HEAD~2").unwrap()).unwrap();

    assert_eq!(plan.len(), 2);
    assert!(plan.iter().all(|e| matches!(e.action, TodoAction::Pick)));
    assert_eq!(plan[0].oid, f.oid("HEAD~1").unwrap());
}

#[test]
fn the_default_plan_carries_each_commit_subject_for_the_editor() {
    let f = test_fixtures::linear(3).unwrap();

    let plan = open(&f).rebase_todo(&f.oid("HEAD~1").unwrap()).unwrap();

    assert_eq!(plan[0].message.as_deref(), Some("commit 2"));
}

#[test]
fn pausing_after_each_commit_inserts_a_break_between_them() {
    let plan = [
        entry("aaaa", TodoAction::Pick),
        entry("bbbb", TodoAction::Pick),
    ];

    let todo = git_engine::render_todo_paused(&plan);

    assert_eq!(todo, "pick aaaa\nbreak\npick bbbb\nbreak\n");
}

#[test]
fn a_dropped_commit_gets_no_break_because_nothing_was_applied() {
    let plan = [
        entry("aaaa", TodoAction::Drop),
        entry("bbbb", TodoAction::Pick),
    ];

    assert_eq!(
        git_engine::render_todo_paused(&plan),
        "drop aaaa\npick bbbb\nbreak\n"
    );
}

#[test]
fn an_edit_step_gets_no_extra_break_because_it_already_stops() {
    let plan = [entry("aaaa", TodoAction::Edit)];
    assert_eq!(git_engine::render_todo_paused(&plan), "edit aaaa\n");
}

#[test]
fn a_paused_rebase_stops_before_it_has_finished() {
    let f = test_fixtures::linear(4).unwrap();
    let base = f.oid("HEAD~2").unwrap();
    let plan = [
        entry(&f.oid("HEAD~1").unwrap(), TodoAction::Pick),
        entry(&f.oid("HEAD").unwrap(), TodoAction::Pick),
    ];

    open(&f).interactive_rebase_paused(&base, &plan).unwrap();

    assert!(f.path().join(".git/rebase-merge").is_dir());
}

#[test]
fn continuing_a_paused_rebase_reaches_the_end() {
    let f = test_fixtures::linear(4).unwrap();
    let base = f.oid("HEAD~2").unwrap();
    let plan = [
        entry(&f.oid("HEAD~1").unwrap(), TodoAction::Pick),
        entry(&f.oid("HEAD").unwrap(), TodoAction::Pick),
    ];
    let repo = open(&f);
    repo.interactive_rebase_paused(&base, &plan).unwrap();

    while f.path().join(".git/rebase-merge").is_dir() {
        repo.continue_operation().unwrap();
    }

    assert_eq!(subjects(&f)[0], "commit 3", "{:?}", subjects(&f));
}
