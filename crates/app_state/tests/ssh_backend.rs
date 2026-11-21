#![allow(clippy::unwrap_used, clippy::print_stdout)]

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::thread;
use std::time::{Duration, Instant};

use app_state::{AppEvent, AppState, BackendState};

const TEST: &str = "ssh_installs_one_agent_for_two_repositories_and_recovers_after_disconnect";
const UPLOAD: &[u8] = b"#!/bin/sh\nexec \"$COGIT_SSH_TEST_BIN\" --agent \"$@\"\n";
const POLL: Duration = Duration::from_millis(20);

fn remote_home() -> PathBuf {
    std::env::var_os("COGIT_SSH_TEST_HOME").unwrap().into()
}

fn script(path: &Path, content: &[u8]) {
    fs::write(path, content).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn fake_ssh(args: &[String]) -> ExitCode {
    let home = remote_home();
    if let Ok(denial) = fs::read(home.join("deny")) {
        std::io::stderr().write_all(&denial).unwrap();
        return ExitCode::from(255);
    }
    let mut connections = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(home.join("connections"))
        .unwrap();
    writeln!(connections, "{}", std::process::id()).unwrap();
    for sequence in [["-p", "2222"], ["-l", "dev"], ["--", "fixture.invalid"]] {
        assert!(args.windows(2).any(|pair| pair == sequence), "{args:?}");
    }
    assert!(args.iter().any(|arg| arg == "-T"));
    let shell = if cfg!(windows) {
        app_state::terminal::bash_of(app_state::desktop::find_git_bash().as_deref())
    } else {
        PathBuf::from("sh")
    };
    let done = Command::new(shell)
        .env_remove("SSH_ASKPASS")
        .env_remove("SSH_ASKPASS_REQUIRE")
        .env_remove("COGIT_ASKPASS_REFUSE")
        .env_remove("COGIT_REPOSITORY_ASKPASS")
        .env_remove("COGIT_SSH_PASSPHRASE")
        .env_remove("COGIT_HOST_KEY_ENDPOINT")
        .env_remove("COGIT_HOST_KEY_TOKEN")
        .current_dir(home)
        .args([
            "-c",
            r#"export HOME="$(pwd)" PATH="$(pwd)/bin:/usr/bin:$PATH"; exec sh -c "$1""#,
            "ssh",
        ])
        .arg(args.last().unwrap())
        .status()
        .unwrap();
    ExitCode::from(done.code().and_then(|c| u8::try_from(c).ok()).unwrap_or(1))
}

fn managed_agent(args: &[String]) -> ExitCode {
    assert_eq!(args, &["serve"]);
    let mut agent = Command::new(env!("CARGO_BIN_EXE_cogit-agent"))
        .env("COGIT_INSIDE_AGENT", "1")
        .args(args)
        .spawn()
        .unwrap();
    let drop_connection = remote_home().join("disconnect");
    loop {
        if let Some(result) = agent.try_wait().unwrap() {
            return if result.success() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
        }
        if drop_connection.exists() {
            agent.kill().unwrap();
            agent.wait().unwrap();
            fs::remove_file(drop_connection).unwrap();
            return ExitCode::from(255);
        }
        thread::sleep(POLL);
    }
}

fn ssh_repositories() {
    let a = test_fixtures::linear(2).unwrap();
    let b = test_fixtures::linear(3).unwrap();
    let home = remote_home();
    let upstream = home.join("upstream.git");
    b.git(&["clone", "--bare", "--", ".", upstream.to_str().unwrap()])
        .unwrap();
    b.git(&["remote", "add", "origin", upstream.to_str().unwrap()])
        .unwrap();
    let address = |path: &Path| {
        assert_eq!(home.parent(), path.parent());
        format!(
            "ssh://dev@fixture.invalid:2222/~/../{}",
            path.file_name().unwrap().to_str().unwrap()
        )
    };
    app_state::provide_agent_binary(UPLOAD);
    let state = AppState::new();
    let _ = state.pulse(Path::new(&address(a.path())));
    assert!(!home.join("connections").exists());
    let first_path = address(a.path());
    for (method, retry) in [("publickey", true), ("password", false)] {
        let diagnostic = format!(
            "{}dev@fixture.invalid: Permission denied ({method}).\n",
            "SSH diagnostic line\n".repeat(42)
        );
        fs::write(home.join("deny"), &diagnostic).unwrap();
        let git_engine::GitError::Command(failure) =
            state.open_repository(Path::new(&first_path)).unwrap_err()
        else {
            panic!("SSH refusal lost its command output");
        };
        assert_eq!(failure.stderr, diagnostic);
        assert_eq!(failure.exit_code, Some(255));
        assert_eq!(
            failure
                .auth_required
                .as_ref()
                .map(|auth| (auth.host.as_str(), auth.ssh)),
            retry.then_some(("fixture.invalid", true))
        );
    }
    fs::remove_file(home.join("deny")).unwrap();
    let left = state
        .open_repository(Path::new(&address(a.path())))
        .unwrap();
    let right = state
        .open_repository(Path::new(&address(b.path())))
        .unwrap();
    assert_ne!(left.repo, right.repo);
    assert!(state.is_remote(left.repo));
    assert_eq!(
        state.agent_process_id(left.repo),
        state.agent_process_id(right.repo)
    );
    let connections = || {
        fs::read_to_string(home.join("connections"))
            .unwrap()
            .lines()
            .count()
    };
    assert_eq!(connections(), 1);
    let installed = home
        .join(".cache/cogit/agent")
        .join(env!("CARGO_PKG_VERSION"))
        .join("cogit-agent");
    assert_eq!(fs::read(&installed).unwrap(), UPLOAD);
    state.show_repository(Some(right.repo));
    let observed_write = |name: &str, text: &str| {
        let mut changes = state.subscribe();
        b.write_file(name, text).unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if matches!(changes.try_recv(), Ok(AppEvent::RepoChanged { repo, .. }) if repo == right.repo)
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "no remote file watcher notification for {name}"
            );
            thread::sleep(POLL);
        }
    };
    observed_write("example.txt", "new contents\n");
    assert_eq!(state.repo_status(right.repo).unwrap().untracked, 1);
    state
        .stage_paths(right.repo, &["example.txt".to_owned()])
        .unwrap();
    assert_eq!(
        b.git(&["diff", "--cached", "--name-only"]).unwrap().trim(),
        "example.txt"
    );
    assert_eq!(state.repo_status(left.repo).unwrap().staged, 0);
    let mut events = state.subscribe();
    fs::write(home.join("disconnect"), []).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        assert!(
            Instant::now() < deadline,
            "no notification for the lost transport"
        );
        if matches!(
            events.try_recv(),
            Ok(AppEvent::Backend {
                state: BackendState::Lost { .. },
                ..
            })
        ) {
            break;
        }
        thread::sleep(POLL);
    }
    fs::write(&installed, b"stale executable").unwrap();
    state.reread_repository(right.repo).unwrap();
    state.reread_repository(left.repo).unwrap();
    assert_eq!(state.repo_status(right.repo).unwrap().staged, 1);
    assert_eq!(state.repo_status(left.repo).unwrap().staged, 0);
    assert_eq!(connections(), 2);
    assert_eq!(fs::read(&installed).unwrap(), UPLOAD);
    observed_write("after-reconnect.txt", "watcher resumed\n");
    let save = home.join("desktop-save");
    let export = home.join("desktop-export");
    assert!(
        state
            .save_blob(right.repo, "HEAD", "file0.txt", &save)
            .unwrap_err()
            .to_string()
            .contains("SSH")
    );
    assert!(
        state
            .export_read_only(right.repo, "HEAD", "file0.txt", &export)
            .unwrap_err()
            .to_string()
            .contains("SSH")
    );
    assert!(!save.exists() && !export.exists());

    let staged = state
        .diff_file(
            right.repo,
            &git_engine::DiffSpec::IndexVsHead,
            "example.txt",
            &diff_engine::DiffOptions::default(),
        )
        .unwrap();
    assert!(matches!(staged, diff_engine::FileDiff::Text { ref hunks, .. } if !hunks.is_empty()));
    let stop = git_engine::NetworkStop::default();
    state
        .fetch(right.repo, "origin", &stop, None, |_| {})
        .unwrap();
    assert_eq!(b.oid("origin/main").unwrap(), b.oid("HEAD").unwrap());
    let committed = state
        .commit(
            right.repo,
            &git_engine::CommitRequest {
                message: "SSH fixture commit".into(),
                only: Vec::new(),
                amend: false,
                signoff: false,
                no_verify: false,
            },
        )
        .unwrap();
    assert_eq!(b.oid("HEAD").unwrap(), committed);
    state
        .push_to(
            right.repo,
            "origin",
            "refs/heads/main:refs/heads/main",
            false,
            &stop,
            None,
            |_| {},
        )
        .unwrap();
    assert_eq!(
        b.git_in(&upstream, &["rev-parse", "main"]).unwrap().trim(),
        committed
    );
    let generation = state.begin_graph();
    state
        .build_graph(
            right.repo,
            &git_engine::CommitQuery::default(),
            generation,
            10,
            |_| true,
        )
        .unwrap();
    let graph = state.graph_window(right.repo, generation, 0, 10).unwrap();
    assert!(graph.complete);
    assert_eq!(graph.commits.len(), 4);

    let revived = "undo through SSH\n";
    a.write_file("file0.txt", revived).unwrap();
    state
        .discard_paths(left.repo, &["file0.txt".into()])
        .unwrap();
    assert_eq!(
        fs::read_to_string(a.path().join("file0.txt")).unwrap(),
        "content 0\n"
    );
    state.undo_last(left.repo).unwrap();
    assert_eq!(
        fs::read_to_string(a.path().join("file0.txt")).unwrap(),
        revived
    );
    assert_eq!(connections(), 2);
    assert!(state.close_repository(left.repo));
    assert!(state.close_repository(right.repo));
}

fn run() {
    let home = test_fixtures::tempdir().unwrap();
    let bin = home.path().join("bin");
    fs::create_dir(&bin).unwrap();
    script(&bin.join("uname"), b"#!/bin/sh\nprintf 'Linux x86_64\\n'\n");
    #[cfg(target_os = "macos")]
    {
        script(&bin.join("sha256sum"), b"#!/bin/sh\nexec shasum -a 256\n");
        script(
            &bin.join("mv"),
            b"#!/bin/sh\nexec /bin/mv -f \"$3\" \"$4\"\n",
        );
    }
    let exe = std::env::current_exe().unwrap();
    let status = Command::new(&exe)
        .arg("--ssh-repositories")
        .env("COGIT_TEST_SSH", &exe)
        .env("COGIT_SSH_TEST_BIN", git_engine::slash_path(&exe))
        .env("COGIT_SSH_TEST_HOME", home.path())
        .status()
        .unwrap();
    assert!(
        status.success(),
        "SSH repository worker exited with {status}"
    );
}

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--agent") => return managed_agent(&args[1..]),
        Some("--ssh-repositories") => ssh_repositories(),
        _ if args.iter().any(|arg| arg == "-T") => return fake_ssh(&args),
        _ if args.iter().any(|arg| arg == "--list") => {
            if !args.iter().any(|arg| arg == "--ignored") {
                println!("{TEST}: test");
            }
        }
        _ if args.is_empty() || args.iter().any(|arg| arg == TEST) => {
            run();
            println!("{TEST} ... ok");
        }
        _ => {}
    }
    ExitCode::SUCCESS
}
