//! User commands (F-954): words, placeholders, the journal and Stop.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::AppState;
use app_state::desktop::Platform;
use app_state::user_commands::{
    ToolArgsError, ToolContext, ToolRequest, Values, expand_args, prepare,
};
use std::path::Path;
use std::time::{Duration, Instant};

fn values(context: &ToolContext) -> Values {
    Values::of(Platform::Windows, Path::new(r"C:\My Repos\app\"), context)
}

fn full() -> ToolContext {
    ToolContext {
        file: Some("src/a b.txt".into()),
        commit: Some("0123456789abcdef0123456789abcdef01234567".into()),
        branch: Some("feature/x".into()),
        ..ToolContext::default()
    }
}

#[test]
fn words_split_before_placeholders_expand_so_a_path_with_spaces_stays_one_argument() {
    let args = expand_args(
        r#"--file ${file} "--title=${branch} at ${shortcommit}" '${relfile}' -C${repo}"#,
        &values(&full()),
    )
    .unwrap();

    assert_eq!(
        args,
        [
            "--file",
            r"C:\My Repos\app\src\a b.txt",
            "--title=feature/x at 0123456",
            "src/a b.txt",
            r"-CC:\My Repos\app",
        ]
    );
}

#[test]
fn quotes_group_words_and_an_empty_pair_is_an_empty_argument() {
    let args = expand_args(r#"log "a b" c""d '' ${commit}"#, &values(&full())).unwrap();
    assert_eq!(
        args,
        [
            "log",
            "a b",
            "cd",
            "",
            "0123456789abcdef0123456789abcdef01234567"
        ]
    );
}

#[test]
fn environment_variables_and_a_lone_dollar_stay_as_written() {
    let args = expand_args("$HOME %PATH% $ ${repo}$", &values(&full())).unwrap();
    assert_eq!(args, ["$HOME", "%PATH%", "$", r"C:\My Repos\app$"]);
}

#[test]
fn an_unknown_or_unclosed_placeholder_is_refused() {
    let context = full();
    assert_eq!(
        expand_args("${home}", &values(&context)),
        Err(ToolArgsError::Unknown("${home}".into()))
    );
    assert_eq!(
        expand_args("${file", &values(&context)),
        Err(ToolArgsError::Unknown("${file".into()))
    );
    assert_eq!(
        expand_args("\"${file}", &values(&context)),
        Err(ToolArgsError::UnbalancedQuote)
    );
}

#[test]
fn a_placeholder_the_context_cannot_fill_is_refused() {
    let repository_only = ToolContext::default();
    for name in ["file", "relfile", "commit", "shortcommit", "line"] {
        assert_eq!(
            expand_args(&format!("x${{{name}}}"), &values(&repository_only)),
            Err(ToolArgsError::Missing(format!("${{{name}}}"))),
        );
    }
    assert_eq!(
        expand_args("${files}", &values(&repository_only)),
        Err(ToolArgsError::Missing("${files}".into())),
    );
    assert_eq!(
        expand_args("${repo}", &values(&repository_only)).unwrap(),
        [r"C:\My Repos\app"]
    );
}

#[test]
fn a_file_list_expands_to_distinct_arguments_and_can_be_quoted() {
    let context = ToolContext {
        files: vec!["src/a b.txt".into(), "docs/draft.md".into()],
        line: Some(17),
        ..ToolContext::default()
    };
    let mut expanded = values(&context);
    expanded.upstream = "origin/main".into();
    expanded.remote = "origin".into();
    expanded.issue = "ABC-42".into();
    assert_eq!(
        expand_args(
            r#""${files}" --root=${root} ${branch} ${upstream} ${remote} ${issue} ${line}"#,
            &expanded
        )
        .unwrap(),
        [
            r"C:\My Repos\app\src\a b.txt",
            r"C:\My Repos\app\docs\draft.md",
            r"--root=C:\My Repos\app",
            "",
            "origin/main",
            "origin",
            "ABC-42",
            "17"
        ]
    );
    assert!(expand_args("--files=${files}", &expanded).is_err());
    assert_eq!(
        expand_args(
            "${branch} ${upstream} ${remote} ${issue}",
            &values(&ToolContext::default())
        )
        .unwrap(),
        ["", "", "", ""]
    );
}

#[test]
fn a_wsl_repository_gets_its_windows_path() {
    let context = ToolContext {
        file: Some("README.md".into()),
        ..ToolContext::default()
    };
    let values = Values::of(
        Platform::Windows,
        Path::new(r"\\wsl.localhost\Ubuntu\home\me\app"),
        &context,
    );
    assert_eq!(
        expand_args("${repo} ${file}", &values).unwrap(),
        [
            r"\\wsl.localhost\Ubuntu\home\me\app",
            r"\\wsl.localhost\Ubuntu\home\me\app\README.md"
        ]
    );
}

fn request(program: &str, args: &str) -> ToolRequest {
    ToolRequest {
        name: "Probe".into(),
        program: program.into(),
        args: args.into(),
        cwd: String::new(),
        detached: false,
        context: ToolContext::default(),
        ..ToolRequest::default()
    }
}

#[test]
fn the_program_and_the_folder_take_quotes_and_placeholders() {
    let mut asked = request(r#""${repo}\tools\run me.exe""#, "");
    asked.cwd = "sub dir".into();
    let (program, args, cwd) = prepare(&asked, &values(&full())).unwrap();
    assert_eq!(program, r"C:\My Repos\app\tools\run me.exe");
    assert!(args.is_empty());
    assert_eq!(cwd, Path::new(r"C:\My Repos\app").join("sub dir"));

    asked.cwd = String::new();
    assert_eq!(
        prepare(&asked, &values(&full())).unwrap().2,
        Path::new(r"C:\My Repos\app")
    );
    assert_eq!(
        prepare(&request("  ", ""), &values(&full())).unwrap_err(),
        ToolArgsError::EmptyProgram
    );
}

#[test]
fn a_captured_run_lands_in_the_journal_with_its_output_and_exit_code() {
    let f = test_fixtures::linear(2).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let stop = git_engine::NetworkStop::default();

    let run = state
        .run_user_command(repo, &request("git", "log -1 --format=%s"), &stop, f.path())
        .unwrap();

    assert_eq!(run.exit_code, Some(0));
    let entry = &state.command_log()[0];
    assert_eq!(entry.id, run.record);
    assert_eq!(entry.operation, "Tool: Probe");
    assert_eq!(entry.command, "git log -1 --format=%s");
    assert_eq!(entry.exit_code, Some(0));
    assert!(!entry.stdout.trim().is_empty(), "{entry:?}");

    let run = state
        .run_user_command(
            repo,
            &request("git", "rev-parse --verify no-such-ref"),
            &stop,
            f.path(),
        )
        .unwrap_err();
    let git_engine::GitError::Command(error) = run else {
        panic!("{run}")
    };
    assert_eq!(error.exit_code, Some(128));
    assert!(error.stderr.contains("fatal:"));
    assert_eq!(error.command, "git rev-parse --verify no-such-ref");
    let entry = &state.command_log()[0];
    assert_eq!(entry.exit_code, Some(128));
    assert!(entry.stderr.contains("fatal:"), "{entry:?}");
}

#[test]
fn a_password_in_a_url_is_not_journaled() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let stop = git_engine::NetworkStop::default();

    let _ = state.run_user_command(
        repo,
        &request("git", "ls-remote https://me:secret@127.0.0.1:1/x.git"),
        &stop,
        f.path(),
    );

    let entry = &state.command_log()[0];
    assert!(!entry.command.contains("secret"), "{}", entry.command);
}

#[test]
fn a_missing_program_says_so() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let stop = git_engine::NetworkStop::default();

    for detached in [false, true] {
        let mut asked = request("cogit-no-such-program-954", "");
        asked.detached = detached;
        let err = state
            .run_user_command(repo, &asked, &stop, f.path())
            .unwrap_err();
        assert!(err.to_string().contains("was not found"), "{err}");
    }
    assert!(state.command_log().is_empty());
}

#[test]
fn a_detached_run_is_journaled_as_started() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let mut asked = request("git", "--version");
    asked.detached = true;

    let run = state
        .run_user_command(repo, &asked, &git_engine::NetworkStop::default(), f.path())
        .unwrap();

    assert_eq!(run.exit_code, Some(0));
    let entry = &state.command_log()[0];
    assert_eq!(entry.command, "git --version");
    assert_eq!(entry.operation, "Tool: Probe");
}

#[test]
fn repository_commands_require_the_current_file_hash_and_run_the_reviewed_definition() {
    let f = test_fixtures::linear(1).unwrap();
    let config = tempfile::tempdir().unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let stop = git_engine::NetworkStop::default();
    let path = ".cogit/commands.json";
    f.write_file(
        path,
        r#"[{"name":"Version","program":"git","args":"--version"}]"#,
    )
    .unwrap();
    let file = state
        .repository_tools(repo, config.path())
        .unwrap()
        .unwrap();
    assert!(!file.trusted);
    let mut asked = request("cogit-no-such-program", "");
    asked.source = Some(app_state::user_commands::RepositoryTool {
        hash: file.hash.clone(),
        index: 0,
    });
    assert!(
        state
            .run_user_command(repo, &asked, &stop, config.path())
            .is_err()
    );
    assert!(state.command_log().is_empty());
    state
        .trust_repository_tools(repo, config.path(), &file.hash)
        .unwrap();
    assert!(
        state
            .repository_tools(repo, config.path())
            .unwrap()
            .unwrap()
            .trusted
    );
    assert_eq!(
        state
            .run_user_command(repo, &asked, &stop, config.path())
            .unwrap()
            .exit_code,
        Some(0)
    );
    assert_eq!(state.command_log()[0].command, "git --version");

    f.write_file(
        path,
        r#"[{"name":"Status","program":"git","args":"status"}]"#,
    )
    .unwrap();
    let changed = state
        .repository_tools(repo, config.path())
        .unwrap()
        .unwrap();
    assert_ne!(file.hash, changed.hash);
    assert!(!changed.trusted);
    assert!(
        state
            .trust_repository_tools(repo, config.path(), &file.hash)
            .is_err()
    );
    assert!(
        state
            .run_user_command(repo, &asked, &stop, config.path())
            .is_err()
    );
    asked.source.as_mut().unwrap().hash = changed.hash;
    assert!(
        state
            .run_user_command(repo, &asked, &stop, config.path())
            .is_err()
    );
    assert_eq!(state.command_log().len(), 1);
}

#[test]
fn confirmation_checks_that_the_expanded_command_is_unchanged() {
    let f = test_fixtures::linear(1).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let stop = git_engine::NetworkStop::default();
    let mut asked = request("git", "--version");
    asked.confirm = true;
    assert!(
        state
            .run_user_command(repo, &asked, &stop, f.path())
            .is_err()
    );
    let preview = state.preview_user_command(repo, &asked, f.path()).unwrap();
    assert_eq!(preview.line, "git --version");
    asked.confirmation = Some(preview.confirmation);
    asked.args = "status".into();
    assert!(
        state
            .run_user_command(repo, &asked, &stop, f.path())
            .is_err()
    );
    assert!(state.command_log().is_empty());
    asked.args = "--version".into();
    assert_eq!(
        state
            .run_user_command(repo, &asked, &stop, f.path())
            .unwrap()
            .exit_code,
        Some(0)
    );
}

#[test]
fn metadata_follows_head_and_the_repository_issue_rule() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["checkout", "-b", "feature/ABC-42-login"]).unwrap();
    f.git(&["remote", "add", "team", "https://example.invalid/repo"])
        .unwrap();
    f.git(&["config", "branch.feature/ABC-42-login.remote", "team"])
        .unwrap();
    f.git(&[
        "config",
        "branch.feature/ABC-42-login.merge",
        "refs/heads/main",
    ])
    .unwrap();
    f.git(&["config", "bugtraq.logregex", "([A-Z]+-[0-9]+)"])
        .unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let metadata = state.tool_metadata(repo, "", true).unwrap();
    assert_eq!(metadata.branch.as_deref(), Some("feature/ABC-42-login"));
    assert_eq!(metadata.upstream, "team/main");
    assert_eq!(metadata.remote, "team");
    assert_eq!(metadata.issue, "ABC-42");
    assert_eq!(
        state
            .tool_metadata(repo, r"feature/ABC-(\d+)", true)
            .unwrap()
            .issue,
        "42"
    );
    f.git(&["checkout", "--detach"]).unwrap();
    assert_eq!(state.tool_metadata(repo, "", true).unwrap().branch, None);
}

#[test]
fn output_modes_keep_full_journal_streams_and_present_only_the_requested_data() {
    use app_state::user_commands::ToolOutput;

    let f = test_fixtures::linear(12).unwrap();
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let stop = git_engine::NetworkStop::default();
    let mut asked = request("git", "log --format=%s");
    for mode in [
        ToolOutput::Panel,
        ToolOutput::Silent,
        ToolOutput::Message,
        ToolOutput::Notification,
    ] {
        asked.output = mode;
        let run = state
            .run_user_command(repo, &asked, &stop, f.path())
            .unwrap();
        let output = &state.command_log()[0];
        assert_eq!(output.stdout.lines().count(), 12);
        match mode {
            ToolOutput::Panel | ToolOutput::Silent => assert_eq!(run.presentation, None),
            ToolOutput::Message => {
                assert_eq!(run.presentation.as_deref(), Some(output.stdout.as_str()))
            }
            ToolOutput::Notification => assert_eq!(run.presentation.unwrap().lines().count(), 8),
        }
    }

    asked.output = ToolOutput::Silent;
    asked.args = r#"-c "alias.fail=!printf out; printf err >&2; exit 7" fail"#.into();
    let failure = state
        .run_user_command(repo, &asked, &stop, f.path())
        .unwrap_err();
    let git_engine::GitError::Command(error) = failure else {
        panic!("{failure}")
    };
    assert_eq!(error.exit_code, Some(7));
    assert_eq!(error.stdout, "out");
    assert_eq!(error.stderr, "err");
}

#[test]
fn export_round_trips_and_an_incomplete_export_cannot_replace_an_existing_file() {
    use app_state::user_commands::{ToolDefinition, ToolOutput, export_commands, import_commands};

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("commands.json");
    let command = ToolDefinition {
        name: "Summary".into(),
        program: "git".into(),
        args: "log -1 --format=%s".into(),
        output: ToolOutput::Message,
        ..ToolDefinition::default()
    };
    export_commands(&path, std::slice::from_ref(&command)).unwrap();
    let loaded = import_commands(&path).unwrap();
    assert_eq!(loaded[0].name, command.name);
    assert_eq!(loaded[0].args, command.args);
    assert_eq!(loaded[0].output, command.output);

    std::fs::write(dir.path().join("commands.json.writing"), "in progress").unwrap();
    assert!(export_commands(&path, &[]).is_err());
    assert_eq!(import_commands(&path).unwrap().len(), 1);
}

#[test]
fn stop_ends_the_whole_process_tree() {
    let f = test_fixtures::linear(1).unwrap();
    let state = std::sync::Arc::new(AppState::new());
    let repo = state.open_repository(f.path()).unwrap().repo;
    // git → sh → a background child and an `exec`; whatever outlives Stop writes `survived`.
    let asked = request(
        "git",
        r#"-c "alias.hang=!(sleep 4; echo > \"$PWD/.git/survived\") & exec sleep 600" hang"#,
    );
    let stop = git_engine::NetworkStop::default();
    let (done, finished) = std::sync::mpsc::channel();
    {
        let (state, stop) = (std::sync::Arc::clone(&state), stop.clone());
        let config_dir = f.path().to_owned();
        std::thread::spawn(move || {
            let _ = done.send(state.run_user_command(repo, &asked, &stop, &config_dir));
        });
    }
    let started = Instant::now();
    while git_engine::children::running() == 0 {
        assert!(
            started.elapsed() < test_fixtures::scaled(Duration::from_secs(20)),
            "the command never started"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(700));

    assert!(stop.stop());
    let run = finished
        .recv_timeout(test_fixtures::scaled(Duration::from_secs(20)))
        .expect("the command was still running after Stop")
        .unwrap();

    assert_eq!(run.exit_code, None);
    assert_eq!(state.command_log()[0].summary, "Canceled by the user");
    std::thread::sleep(test_fixtures::scaled(Duration::from_secs(6)));
    assert!(
        !f.path().join(".git/survived").exists(),
        "a process the command started outlived Stop"
    );
}
