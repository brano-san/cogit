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
    for name in ["file", "relfile", "commit", "shortcommit", "branch"] {
        assert_eq!(
            expand_args(&format!("x${{{name}}}"), &values(&repository_only)),
            Err(ToolArgsError::Missing(format!("${{{name}}}"))),
        );
    }
    assert_eq!(
        expand_args("${repo}", &values(&repository_only)).unwrap(),
        [r"C:\My Repos\app"]
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
        .run_user_command(repo, &request("git", "log -1 --format=%s"), &stop)
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
        )
        .unwrap();
    assert_eq!(run.exit_code, Some(128));
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
        let err = state.run_user_command(repo, &asked, &stop).unwrap_err();
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
        .run_user_command(repo, &asked, &git_engine::NetworkStop::default())
        .unwrap();

    assert_eq!(run.exit_code, Some(0));
    let entry = &state.command_log()[0];
    assert_eq!(entry.command, "git --version");
    assert_eq!(entry.operation, "Tool: Probe");
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
        std::thread::spawn(move || {
            let _ = done.send(state.run_user_command(repo, &asked, &stop));
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
