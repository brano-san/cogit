#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, Host};
use git_engine::WorktreeView;
use serde::Serialize;
use serde_json::Value;

fn loopback() -> Host {
    Host::Loopback(env!("CARGO_BIN_EXE_cogit-agent").into())
}

fn json(value: &impl Serialize) -> Value {
    serde_json::to_value(value).unwrap()
}

fn without_id(summary: &app_state::RepoSummary) -> Value {
    let mut value = json(summary);
    value.as_object_mut().unwrap().remove("repo");
    value
}

fn changed() -> test_fixtures::Fixture {
    let f = test_fixtures::linear(3).unwrap();
    f.write_file("file0.txt", "edited\n").unwrap();
    f.write_file("new.txt", "new\n").unwrap();
    f.git(&["add", "new.txt"]).unwrap();
    f
}

#[test]
fn a_repository_opened_through_the_agent_reads_as_the_local_one() {
    let f = changed();
    let (local, remote) = (AppState::new(), AppState::new());
    let near = local.open_repository(f.path()).unwrap();
    let far = remote.open_repository_on(&loopback(), f.path()).unwrap();

    assert!(remote.is_remote(far.repo));
    assert_eq!(local.is_remote(near.repo), cfg!(feature = "loopback"));
    assert_eq!(without_id(&far), without_id(&near));
    assert_eq!(
        json(&remote.repo_status(far.repo).unwrap()),
        json(&local.repo_status(near.repo).unwrap())
    );
    assert_eq!(
        json(
            &remote
                .worktree_files(far.repo, WorktreeView::default())
                .unwrap()
        ),
        json(
            &local
                .worktree_files(near.repo, WorktreeView::default())
                .unwrap()
        )
    );
    assert_eq!(
        json(&remote.repo_refs(far.repo).unwrap()),
        json(&local.repo_refs(near.repo).unwrap())
    );
    let row = |state: &AppState| {
        let mut rows = json(&state.overviews());
        rows[0].as_object_mut().unwrap().remove("repo");
        rows
    };
    assert_eq!(row(&remote), row(&local));
    assert_eq!(row(&remote)[0]["dirty"], true);
}

#[test]
fn an_error_in_the_agent_arrives_as_the_same_git_error() {
    let folder = test_fixtures::tempdir().unwrap();
    let near = AppState::new().open_repository(folder.path()).unwrap_err();
    let far = AppState::new()
        .open_repository_on(&loopback(), folder.path())
        .unwrap_err();
    assert_eq!(json(&far), json(&near));
}

#[test]
fn local_and_agent_repositories_keep_their_own_ids() {
    let (one, two, three) = (
        test_fixtures::linear(1).unwrap(),
        changed(),
        test_fixtures::linear(2).unwrap(),
    );
    let state = AppState::new();
    let host = loopback();
    let near = state.open_repository(one.path()).unwrap();
    let first = state.open_repository_on(&host, two.path()).unwrap();
    let second = state.open_repository_on(&host, three.path()).unwrap();

    let ids = [near.repo, first.repo, second.repo];
    assert_eq!(
        ids.len(),
        ids.iter().collect::<std::collections::HashSet<_>>().len()
    );
    assert_eq!(state.repo_status(near.repo).unwrap().unstaged, 0);
    assert_eq!(state.repo_status(first.repo).unwrap().unstaged, 1);
    assert_eq!(state.repo_status(second.repo).unwrap().unstaged, 0);
    assert_eq!(state.get(first.repo).unwrap().root, dunce_root(two.path()));
}

#[test]
fn a_closed_agent_repository_answers_like_a_closed_local_one() {
    let f = changed();
    let state = AppState::new();
    let far = state.open_repository_on(&loopback(), f.path()).unwrap();
    assert!(state.close_repository(far.repo));
    assert!(!state.is_remote(far.repo));

    let near = AppState::new();
    let gone = near.open_repository(f.path()).unwrap();
    near.close_repository(gone.repo);
    assert_eq!(
        json(&state.repo_status(far.repo).unwrap_err()),
        json(&near.repo_status(gone.repo).unwrap_err())
    );
}

fn dunce_root(path: &std::path::Path) -> std::path::PathBuf {
    let state = AppState::new();
    let repo = state.open_repository(path).unwrap().repo;
    state.get(repo).unwrap().root
}

const BY_HAND: [&str; 13] = [
    "build_graph",
    "graph_window_wire",
    "search_contents",
    "origin_candidates",
    "scan_worktree_removal",
    "show_repository",
    "undo_last",
    "undo_entry",
    "open_submodule",
    "reread_repository",
    "open_worktree",
    "config_file",
    "save_config_file",
];

const ON_WINDOWS: [(&str, &str); 7] = [
    ("agent_process_id", "the agent process itself"),
    ("is_remote", "the routing table itself"),
    ("close_repository", "the registry; tells the agent itself"),
    ("unregister", "the registry"),
    ("get", "the registry"),
    ("root_of", "the registry"),
    ("forget_row", "the Windows tree-row cache"),
];

const LATER: [(&str, &str); 21] = [
    (
        "presets_for",
        "the preset library is on Windows; settings reach the agent: step 6",
    ),
    (
        "export_preset",
        "the preset library is on Windows; settings reach the agent: step 6",
    ),
    (
        "install_preset",
        "the preset library is on Windows; settings reach the agent: step 6",
    ),
    ("fetch", "streams git's output, sign-in: steps 4 and 6"),
    ("pull", "streams git's output, sign-in: steps 4 and 6"),
    ("push", "streams git's output, sign-in: steps 4 and 6"),
    ("fetch_more", "streams git's output, sign-in: steps 4 and 6"),
    (
        "fetch_depth",
        "streams git's output, sign-in: steps 4 and 6",
    ),
    ("push_to", "streams git's output, sign-in: steps 4 and 6"),
    ("fetch_with", "streams git's output, sign-in: steps 4 and 6"),
    ("pull_with", "streams git's output, sign-in: steps 4 and 6"),
    ("push_with", "streams git's output, sign-in: steps 4 and 6"),
    (
        "push_dry_run",
        "streams git's output, sign-in: steps 4 and 6",
    ),
    ("push_notes", "streams git's output, sign-in: steps 4 and 6"),
    ("delete_remote_branch", "sign-in: step 6"),
    ("delete_remote_tag", "sign-in: step 6"),
    ("delete_refs", "sign-in: step 6"),
    ("move_to_trash", "the Windows recycle bin callback: step 6"),
    ("start_merge_tool", "a Windows program on the files: step 6"),
    ("cancel_merge_tool", "with start_merge_tool: step 6"),
    ("merge_tools_running", "with start_merge_tool: step 6"),
];

fn repo_methods() -> Vec<String> {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(src).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(path).unwrap();
        for piece in text.split("pub fn ").skip(1) {
            let name: String = piece
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            let params = piece.split(")").next().unwrap_or_default();
            if !name.is_empty() && params.contains("RepoId") && params.contains("&self") {
                found.push(name);
            }
        }
    }
    found
}

fn routed() -> Vec<String> {
    let routes = include_str!("../src/routes.rs");
    let list = routes.split("routed! {").nth(1).unwrap();
    list.lines()
        .filter_map(|line| line.trim().strip_prefix("fn "))
        .map(|rest| rest.split('(').next().unwrap().to_owned())
        .collect()
}

#[test]
fn every_repository_method_goes_through_the_agent_or_says_why_not() {
    let routed = routed();
    let accounted = |name: &str| {
        routed.iter().any(|r| r == name)
            || BY_HAND.contains(&name)
            || ON_WINDOWS.iter().any(|(n, _)| *n == name)
            || LATER.iter().any(|(n, _)| *n == name)
    };
    let mut strays: Vec<String> = repo_methods()
        .into_iter()
        .filter(|name| !accounted(name))
        .collect();
    strays.sort();
    strays.dedup();
    assert!(
        strays.is_empty(),
        "INV-13: a method taking RepoId goes into routed! (src/routes.rs) or into a list of \
         this test with the reason it stays on Windows: {strays:?}"
    );
    assert!(
        routed.len() > 150,
        "routed! lost its list: {}",
        routed.len()
    );
}

fn graph_of(state: &AppState, repo: app_state::RepoId) -> (Value, Vec<Value>) {
    let generation = state.begin_graph();
    let mut steps = Vec::new();
    let skipped = state
        .build_graph(
            repo,
            &git_engine::CommitQuery::default(),
            generation,
            7,
            |progress| {
                steps.push(json(&progress));
                true
            },
        )
        .unwrap();
    let window = state.graph_window(repo, generation, 0, 100).unwrap();
    assert!(window.complete);
    (json(&(skipped, window)), steps)
}

#[test]
fn a_graph_walked_in_the_agent_arrives_as_the_local_one() {
    let f = test_fixtures::linear(30).unwrap();
    let (local, remote) = (AppState::new(), AppState::new());
    let near = local.open_repository(f.path()).unwrap().repo;
    let far = remote
        .open_repository_on(&loopback(), f.path())
        .unwrap()
        .repo;

    let (near_graph, near_steps) = graph_of(&local, near);
    let (far_graph, far_steps) = graph_of(&remote, far);
    assert_eq!(far_graph, near_graph);
    assert_eq!(far_steps, near_steps);
    assert!(far_steps.len() > 1, "{far_steps:?}");
}

#[test]
fn a_graph_the_window_gives_up_on_stops_in_the_agent() {
    let f = test_fixtures::linear(200).unwrap();
    let state = AppState::new();
    let repo = state
        .open_repository_on(&loopback(), f.path())
        .unwrap()
        .repo;
    let generation = state.begin_graph();
    let mut heard = 0;
    state
        .build_graph(
            repo,
            &git_engine::CommitQuery::default(),
            generation,
            1,
            |_| {
                heard += 1;
                false
            },
        )
        .unwrap();
    assert_eq!(heard, 1);
    let (graph, _) = graph_of(&state, repo);
    assert!(graph.to_string().contains("\"complete\":true"));
}

#[test]
fn a_command_and_a_change_in_the_agent_reach_the_window() {
    let f = changed();
    let state = AppState::new();
    let repo = state
        .open_repository_on(&loopback(), f.path())
        .unwrap()
        .repo;
    state.show_repository(Some(repo));
    let mut events = state.subscribe();

    state.stage_paths(repo, &["file0.txt".to_owned()]).unwrap();
    let rows = state.command_rows();
    assert!(
        rows.iter().any(|row| row.command.contains("add")),
        "{rows:?}"
    );
    let mut recorded = false;
    let mut changed = false;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while state.watcher_is_quiet(repo) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    f.write_file("later.txt", "later\n").unwrap();
    while !(recorded && changed) && std::time::Instant::now() < deadline {
        match events.try_recv() {
            Ok(app_state::AppEvent::CommandRecorded(notice)) => {
                recorded |= state.command_outcome(notice.id).is_some();
            }
            Ok(app_state::AppEvent::RepoChanged {
                repo: changed_repo, ..
            }) => {
                changed |= changed_repo == repo;
            }
            Ok(_) => {}
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(20)),
        }
    }
    assert!(recorded, "no command reached the window's journal");
    assert!(
        changed,
        "no change reached the window from the agent's watcher"
    );
}

fn kill(pid: u32) {
    let status = if cfg!(windows) {
        std::process::Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .output()
    } else {
        std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output()
    };
    assert!(status.unwrap().status.success());
}

#[test]
fn a_lost_agent_is_told_and_a_refresh_starts_it_again() {
    let f = changed();
    let state = AppState::new();
    let repo = state
        .open_repository_on(&loopback(), f.path())
        .unwrap()
        .repo;
    let mut events = state.subscribe();
    kill(state.agent_process_id(repo).unwrap());

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let reason = loop {
        assert!(
            std::time::Instant::now() < deadline,
            "no word of the lost agent"
        );
        match events.try_recv() {
            Ok(app_state::AppEvent::Backend {
                state: app_state::BackendState::Lost { reason },
                ..
            }) => break reason,
            Ok(_) => {}
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(20)),
        }
    };
    assert!(reason.contains("exited"), "{reason}");
    let lost = state.repo_status(repo).unwrap_err();
    assert!(
        matches!(&lost, git_engine::GitError::Unavailable(text) if text.contains("F5")),
        "{lost:?}"
    );

    state.reread_repository(repo).unwrap();
    assert_eq!(state.repo_status(repo).unwrap().unstaged, 1);
}

#[test]
#[ignore = "needs WSL with git and a musl cogit-agent in COGIT_AGENT_BIN; distro in COGIT_TEST_DISTRO"]
fn a_repository_in_wsl_opens_through_its_agent() {
    let agent = std::fs::read(std::env::var("COGIT_AGENT_BIN").unwrap()).unwrap();
    app_state::provide_agent_binary(Box::leak(agent.into_boxed_slice()));
    let distro = std::env::var("COGIT_TEST_DISTRO").unwrap_or_else(|_| "Ubuntu-22.04".to_owned());
    let folder = format!("/tmp/cogit-wsl-test-{}", std::process::id());
    let made = std::process::Command::new("wsl.exe")
        .args(["-d", &distro, "-e", "sh", "-c"])
        .arg(r#"set -e; rm -rf "$1"; mkdir -p "$1"; cd "$1"; git init -q; echo a > a.txt; echo b > b.txt; git add b.txt"#)
        .args(["sh", &folder])
        .output()
        .unwrap();
    assert!(made.status.success(), "{made:?}");

    let state = AppState::new();
    let path = format!(r"\\wsl.localhost\{distro}{}", folder.replace('/', r"\"));
    let summary = state.open_repository(std::path::Path::new(&path)).unwrap();
    assert!(state.is_remote(summary.repo));
    assert_eq!(summary.root, path.replace('\\', "/"));
    let status = state.repo_status(summary.repo).unwrap();
    assert_eq!((status.staged, status.untracked), (1, 1), "{status:?}");

    let mut events = state.subscribe();
    let killed = std::process::Command::new("wsl.exe")
        .args(["-d", &distro, "-e", "pkill", "-9", "-x", "cogit-agent"])
        .output()
        .unwrap();
    assert!(killed.status.success(), "{killed:?}");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !matches!(
        events.try_recv(),
        Ok(app_state::AppEvent::Backend {
            state: app_state::BackendState::Lost { .. },
            ..
        })
    ) {
        assert!(
            std::time::Instant::now() < deadline,
            "no word of the lost agent"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(matches!(
        state.repo_status(summary.repo),
        Err(git_engine::GitError::Unavailable(_))
    ));
    state.reread_repository(summary.repo).unwrap();
    assert_eq!(state.repo_status(summary.repo).unwrap().untracked, 1);
    assert!(state.close_repository(summary.repo));
}
