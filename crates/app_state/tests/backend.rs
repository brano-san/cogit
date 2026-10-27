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

const BY_HAND: [&str; 7] = [
    "undo_last",
    "undo_entry",
    "open_submodule",
    "reread_repository",
    "open_worktree",
    "config_file",
    "save_config_file",
];

const ON_WINDOWS: [(&str, &str); 7] = [
    ("is_remote", "the routing table itself"),
    ("close_repository", "the registry; tells the agent itself"),
    ("unregister", "the registry"),
    ("get", "the registry"),
    ("root_of", "the registry"),
    ("forget_row", "the Windows tree-row cache"),
    ("show_repository", "which repository the window shows"),
];

const LATER: [(&str, &str); 33] = [
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
    ("build_graph", "streams chunks: step 4"),
    ("graph_window", "reads the graph build_graph left: step 4"),
    (
        "graph_window_wire",
        "reads the graph build_graph left: step 4",
    ),
    (
        "graph_working_tree",
        "reads the graph build_graph left: step 4",
    ),
    ("graph_row_of", "reads the graph build_graph left: step 4"),
    (
        "graph_texts_read",
        "reads the graph build_graph left: step 4",
    ),
    (
        "graph_footprint",
        "reads the graph build_graph left: step 4",
    ),
    ("graph_overlay", "reads the graph build_graph left: step 4"),
    ("search_contents", "streams batches: step 4"),
    ("origin_candidates", "a cancel callback: step 4"),
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
    ("scan_worktree_removal", "streams chunks: step 4"),
    (
        "watcher_is_quiet",
        "the watcher moves into the agent: step 4",
    ),
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
