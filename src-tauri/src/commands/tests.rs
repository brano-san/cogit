/// A command declared `pub fn` is `ExecutionContext::Blocking`: it runs inline on the
/// thread that delivered the IPC message, which on Windows is the thread pumping
/// window messages. Anything slow there stops the window redrawing while it runs
/// (problem 1). Only work that *must* stay on the main thread, or that is a handful of
/// memory reads, belongs in this list.
const MAIN_THREAD_ONLY: &[&str] = &[
    "default_keymap",
    "set_keymap",
    "set_menu_state",
    "report_timing",
    "log_from_frontend",
    "report_memory",
    "closing_ping",
    "cancel_operation",
    "terminal_choices",
    "command_log",
    "command_problems",
    "clear_command_log",
    "safety_log",
    "popup_context_menu",
    "merge_resolved",
];

/// Each command with a flag: does it leave the main thread? An `async fn` does, and so
/// does a plain `fn` marked `#[tauri::command(async)]` — tauri hands that one to the
/// thread pool without demanding it return a `Result`.
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
}

/// Every command in `source` with its body: everything up to the next attribute.
fn commands_in(source: &str) -> Vec<Command> {
    let mut found: Vec<Command> = Vec::new();
    let mut armed: Option<bool> = None;
    let mut open = false;
    for line in source.lines() {
        let line = line.trim_start();
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
            });
            armed = None;
            open = true;
        } else if open && let Some(command) = found.last_mut() {
            command.body.push_str(line);
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
    const WINDOW_WORK: &[&str] = &["child_window::", "WebviewWindowBuilder", "window.close()"];

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
    ];
    let all = all_commands();
    let unqueued: Vec<&str> = WRITERS
        .iter()
        .copied()
        .filter(|name| {
            !all.iter()
                .any(|command| command.name == *name && command.body.contains("mutating("))
        })
        .collect();

    assert!(
        unqueued.is_empty(),
        "these write outside the lane: {unqueued:?}"
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
        "these commands block the window's message loop; mark them \
         `#[tauri::command(async)]` or justify them in MAIN_THREAD_ONLY: {stragglers:?}"
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
