/// A command declared `pub fn` is `ExecutionContext::Blocking`: it runs inline on the
/// thread that delivered the IPC message, which on Windows is the thread pumping
/// window messages. Anything slow there stops the window redrawing while it runs
/// (problem 1). Only work that *must* stay on the main thread, or that is a handful of
/// memory reads, belongs in this list.
const MAIN_THREAD_ONLY: &[&str] = &[
    "default_keymap",
    "set_keymap",
    "capture_keys",
    "set_menu_state",
    "report_timing",
    "log_from_frontend",
    "report_memory",
    "closing_ping",
    "cancel_operation",
    "terminal_choices",
    "command_problems",
    "clear_command_log",
    "safety_log",
    "popup_context_menu",
    "window_chrome",
    "menu_model",
];

/// Each command with a flag: does it leave the main thread? An `async fn` does, and so
/// does a plain `fn` marked `#[tauri::command(async)]` — but that one runs on a tokio
/// worker, one of those that carry IPC, not in the pool of blocking threads: anything slow
/// in it still goes through `blocking(...)`.
/// Every file of the module, read from disk: a list of files here missed `toolbar.rs`.
fn declared() -> Vec<(String, bool)> {
    all_commands()
        .into_iter()
        .map(|command| (command.name, command.off_thread))
        .collect()
}

fn name_of(rest: &str) -> String {
    rest.split(['(', '<']).next().unwrap_or_default().to_owned()
}

struct Command {
    name: String,
    off_thread: bool,
    /// Lines joined, so that a chained call reads as one.
    body: String,
    /// Lines from the signature to the closing brace at column 0.
    lines: usize,
    /// `body` cut at that brace: no helper or doc comment after the command.
    own: String,
}

/// Every command in `source` with its body: everything up to the next attribute.
fn commands_in(source: &str) -> Vec<Command> {
    let mut found: Vec<Command> = Vec::new();
    let mut armed: Option<bool> = None;
    let mut open = false;
    let mut counting = false;
    for raw in source.lines() {
        let line = raw.trim_start();
        if let Some(rest) = line.strip_prefix("#[tauri::command") {
            armed = Some(rest.starts_with("(async)"));
            open = false;
            continue;
        }
        if line.starts_with("#[cfg(test)]") {
            armed = None;
            open = false;
            continue;
        }
        let signature = line
            .strip_prefix("pub async fn ")
            .map(|rest| (rest, true))
            .or_else(|| line.strip_prefix("pub fn ").map(|rest| (rest, false)));
        if let (Some(marked_async), Some((rest, is_async))) = (armed, signature) {
            found.push(Command {
                name: name_of(rest),
                off_thread: is_async || marked_async,
                body: String::new(),
                lines: 1,
                own: rest.to_owned(),
            });
            armed = None;
            open = true;
            counting = true;
        } else if open && let Some(command) = found.last_mut() {
            command.body.push_str(line);
            if counting {
                command.lines += 1;
                command.own.push_str(line);
                counting = raw != "}";
            }
        }
    }
    found
}

/// Every file of this module, so a command moved out of `mod.rs` is still checked.
fn all_commands() -> Vec<Command> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/commands");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "rs") {
            found.extend(commands_in(&std::fs::read_to_string(path).unwrap()));
        }
    }
    found
}

/// `WebviewWindowBuilder::build` deadlocks on Windows inside the WebView2 callback that
/// runs a synchronous command (wry#583): the window appears without its webview, and
/// no window gets an IPC answer again (#8, doc/12-risks.md R-201).
#[test]
fn a_window_is_never_opened_or_closed_on_the_main_thread() {
    const WINDOW_WORK: &[&str] = &[
        "child_window::",
        "reveal_or_open(",
        "WebviewWindowBuilder",
        "window.close()",
    ];

    let stuck: Vec<String> = all_commands()
        .into_iter()
        .filter(|command| WINDOW_WORK.iter().any(|call| command.body.contains(call)))
        .filter(|command| !command.off_thread)
        .map(|command| command.name)
        .collect();

    assert!(
        stuck.is_empty(),
        "these commands build or close a window on the main thread; make them async: {stuck:?}"
    );
}

#[test]
fn the_body_parser_sees_the_window_commands() {
    let all = all_commands();
    let body_of = |name: &str| {
        all.iter()
            .find(|command| command.name == name)
            .map(|command| command.body.as_str())
            .unwrap_or_default()
    };
    assert!(body_of("open_compare_window").contains("child_window::open"));
    assert!(body_of("close_this_window").contains("window.close()"));
    assert!(body_of("open_solver_window").contains("reveal_or_open("));
    assert!(body_of("open_errors_window").contains("reveal_or_open("));
}

#[test]
fn a_synchronous_window_command_would_be_caught() {
    // Assembled, so the capability check that scans these files sees no real call here.
    let source = format!(
        "#[tauri::command]\n#[specta::specta]\npub fn open_it(app: AppHandle) {{\n    crate::{}::open(&app, \"x\");\n}}\n",
        "child_window"
    );
    let found = commands_in(&source);
    assert_eq!(found.len(), 1);
    assert!(!found[0].off_thread);
    assert!(found[0].body.contains("child_window::"));
}

#[test]
fn a_synchronous_reveal_or_open_command_would_be_caught() {
    let source = format!(
        "#[tauri::command]
#[specta::specta]
pub fn open_it(app: AppHandle) {{
    crate::errors_window::{}(&app);
}}
",
        "reveal_or_open"
    );
    let found = commands_in(&source);
    assert_eq!(found.len(), 1);
    assert!(!found[0].off_thread);
    assert!(found[0].body.contains("reveal_or_open("));
}

// Toggling the exec bit while a commit held index.lock failed with "index.lock exists",
// and saving the Git config beside a queued `set_upstream` with "could not lock config
// file": these wrote the repository without waiting for its lane.
#[test]
fn commands_that_write_the_repository_wait_for_its_lane() {
    const WRITERS: &[&str] = &[
        "stage_mode",
        "write_git_config",
        "write_hook",
        "set_hook_enabled",
        "use_hooks_path",
        "move_to_trash",
        "install_preset",
        "run_hook",
        "run_check",
        "rename_remote",
        "remove_remote",
        "set_remote_properties",
    ];
    let all = all_commands();
    let unqueued: Vec<&str> = WRITERS
        .iter()
        .copied()
        .filter(|name| {
            !all.iter().any(|command| {
                command.name == *name
                    && (command.body.contains("mutating(")
                        || command.body.contains("mutating_titled("))
            })
        })
        .collect();

    assert!(
        unqueued.is_empty(),
        "these write outside the lane: {unqueued:?}"
    );
}

// The footer's cancel reaches only a command registered by its queue operation (03 §3 п.6).
#[test]
fn every_command_that_talks_to_a_remote_can_be_cancelled() {
    let all = all_commands();
    let registered = |body: &str| body.contains("networking(") || body.contains("network_stop(");
    let uncancellable: Vec<&str> = [
        "fetch",
        "pull",
        "push",
        "push_to",
        "fetch_more",
        "fetch_depth",
        "clone_repository",
    ]
    .into_iter()
    .filter(|name| {
        !all.iter()
            .any(|command| command.name == *name && registered(&command.body))
    })
    .collect();
    assert!(
        uncancellable.is_empty(),
        "cancel_network cannot stop these: {uncancellable:?}"
    );
}

// `#[tauri::command(async)]` on a plain fn runs it on a tokio worker, the ones that
// carry IPC; reading every listed repository there stalled other commands after fetch.
#[test]
fn the_repository_list_is_read_off_the_async_workers() {
    let all = all_commands();
    let list = all.iter().find(|command| command.name == "repositories");
    assert!(list.is_some_and(|command| command.body.contains("blocking(")));
}

// The keyring is a system service: Secret Service waits on D-Bus for the keyring to unlock,
// and that wait held an IPC worker.
#[test]
fn the_keyring_is_reached_off_the_async_workers() {
    let all = all_commands();
    let inline: Vec<&str> = ["has_token", "store_token", "forget_token"]
        .into_iter()
        .filter(|name| {
            !all.iter()
                .any(|command| command.name == *name && command.body.contains("blocking("))
        })
        .collect();
    assert!(
        inline.is_empty(),
        "these reach the keyring inline: {inline:?}"
    );
}

// The whole journal is cloned and serialised on every `command-recorded` while Output is
// open: up to a hundred entries of a megabyte each, on the thread that paints the window.
#[test]
fn the_command_log_is_read_off_the_main_thread() {
    let log = declared()
        .into_iter()
        .find(|(name, _)| name == "command_log");
    assert_eq!(log, Some(("command_log".to_owned(), true)));
}

// The footer said "Checking out" for a reset, "Undoing" for a rollback and "Committing" for
// a split: the title of the kind, not of what ran (BE-016).
#[test]
fn the_footer_names_what_runs() {
    let all = all_commands();
    let wrong: Vec<&str> = [
        ("reset_to", "\"Resetting\""),
        ("rollback_to", "\"Rolling back\""),
        ("split_off", "\"Splitting\""),
    ]
    .into_iter()
    .filter(|(name, title)| {
        !all.iter().any(|command| {
            command.name == *name
                && command.body.contains("mutating_titled(")
                && command.body.contains(title)
        })
    })
    .map(|(name, _)| name)
    .collect();
    assert!(wrong.is_empty(), "the footer mislabels: {wrong:?}");
}

// `#[tauri::command(async)]` on these ran the journal's clone and serialisation, and the
// settings file's read and rename, on the workers that carry every other command's answer.
#[test]
fn the_journal_and_the_settings_are_read_in_the_blocking_pool() {
    let all = all_commands();
    let inline: Vec<&str> = [
        "command_log",
        "command_log_text",
        "command_outcome",
        "read_settings",
        "write_setting",
    ]
    .into_iter()
    .filter(|name| {
        !all.iter()
            .any(|command| command.name == *name && command.body.contains("blocking"))
    })
    .collect();
    assert!(inline.is_empty(), "these run on an IPC worker: {inline:?}");
}

/// Repository commands allowed more than one call into `AppState` or a longer body than
/// INV-09's "get the state, call one method, map the error".
const ORCHESTRATING: &[(&str, &str)] = &[
    (
        "commit",
        "maintenance after a commit is its own queued step",
    ),
    (
        "load_commits",
        "the generation is taken before the walk leaves the IPC worker",
    ),
    (
        "close_repository",
        "the overviews drop the closed repository too",
    ),
    ("search_file_contents", "frames the matches into a channel"),
    ("origin_candidates", "frames the candidates into a channel"),
    (
        "scan_worktree_removal",
        "frames the scan into a cancellable channel",
    ),
    ("investigate_blame", "frames the report into a channel"),
    (
        "launch_merge_tool",
        "announces the tool's end as an app event",
    ),
];

const THIN_LINES: usize = 30;

/// Distinct `AppState` methods a body calls, by the two names commands give it.
fn state_calls(body: &str) -> std::collections::BTreeSet<&str> {
    ["app_state.", "state.state."]
        .iter()
        .flat_map(|receiver| {
            body.match_indices(receiver)
                .map(|(at, _)| &body[at + receiver.len()..])
        })
        .filter_map(|rest| {
            let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_'))?;
            rest[end..].starts_with('(').then(|| &rest[..end])
        })
        .filter(|method| !method.is_empty() && *method != "clone")
        .collect()
}

fn repo_scoped(command: &Command) -> bool {
    command.own.contains("repo: RepoId") || command.own.contains("owner: RepoId")
}

fn fat(command: &Command) -> bool {
    repo_scoped(command) && (state_calls(&command.own).len() > 1 || command.lines > THIN_LINES)
}

#[test]
fn repo_commands_stay_thin() {
    let fat: Vec<String> = all_commands()
        .into_iter()
        .filter(fat)
        .filter(|command| !ORCHESTRATING.iter().any(|(name, _)| *name == command.name))
        .map(|command| {
            let calls = state_calls(&command.own);
            format!(
                "{} ({} lines, calls {calls:?})",
                command.name, command.lines
            )
        })
        .collect();

    assert!(
        fat.is_empty(),
        "INV-09: a repository command gets the state, calls one method and maps the error; \
         move the rest into a crate or justify it in ORCHESTRATING: {fat:?}"
    );
}

#[test]
fn the_orchestrating_list_has_no_leftovers() {
    let all = all_commands();
    let thin: Vec<&str> = ORCHESTRATING
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| {
            !all.iter()
                .any(|command| command.name == *name && fat(command))
        })
        .collect();

    assert!(thin.is_empty(), "thin now or gone, drop them: {thin:?}");
}

#[test]
fn the_thin_check_counts_calls_and_lines() {
    let source = "#[tauri::command]\npub async fn two(\n    repo: RepoId,\n) {\n    let app_state = state.state.clone();\n    app_state.open(repo);\n    state.state.close(repo);\n}\n";
    let found = commands_in(source);
    assert_eq!(found[0].lines, 7);
    assert!(fat(&found[0]));
    assert_eq!(
        state_calls(&found[0].own).into_iter().collect::<Vec<_>>(),
        ["close", "open"]
    );
}

#[test]
fn the_parser_sees_every_command() {
    let all = declared();
    assert!(
        all.len() > 60,
        "expected the whole command surface, parsed {}",
        all.len()
    );
    assert!(all.iter().any(|(name, _)| name == "repositories"));
    assert!(all.iter().any(|(name, _)| name == "delete_merged_branches"));
}

#[test]
fn nothing_heavy_runs_on_the_main_thread() {
    let stragglers: Vec<String> = declared()
        .into_iter()
        .filter(|(name, off_thread)| !off_thread && !MAIN_THREAD_ONLY.contains(&name.as_str()))
        .map(|(name, _)| name)
        .collect();

    assert!(
        stragglers.is_empty(),
        "these commands block the window's message loop; make them `async fn` with the work \
         in `blocking(...)`, or justify them in MAIN_THREAD_ONLY: {stragglers:?}"
    );
}

#[test]
fn the_main_thread_list_has_no_leftovers() {
    let names: Vec<String> = declared().into_iter().map(|(name, _)| name).collect();
    let gone: Vec<&&str> = MAIN_THREAD_ONLY
        .iter()
        .filter(|allowed| !names.iter().any(|name| name == *allowed))
        .collect();

    assert!(gone.is_empty(), "no longer commands at all: {gone:?}");
}
