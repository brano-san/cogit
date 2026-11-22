#![allow(clippy::unwrap_used, clippy::print_stdout)]

use app_state::{AppEvent, AppState, OperationKind};
use git_engine::{GitError, NetworkStop, RepoHandle};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

const NEW_HOST: &str = "The authenticity of host 'h.example (192.0.2.1)' can't be established.\nED25519 key fingerprint is SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU.\nThis key is not known by any other names.\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ";
const CHANGED: &str = "Warning: the ED25519 host key for 'h.example' differs from the key for the IP address '192.0.2.9'\nOffending key for IP in /home/ann/.ssh/known_hosts:5\nAre you sure you want to continue connecting (yes/no)? ";
const PASSWORD: &str = "git@h.example's password: ";

fn state_answering(accept: bool) -> (Arc<AppState>, Arc<AtomicUsize>) {
    let (state, asks) = window_answering(accept);
    state.ask_host_keys_here();
    (state, asks)
}

fn window_answering(accept: bool) -> (Arc<AppState>, Arc<AtomicUsize>) {
    let state = Arc::new(AppState::new());
    let asks = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&asks);
    let mut heard = state.subscribe();
    let answering = Arc::clone(&state);
    std::thread::spawn(move || {
        while let Ok(event) = heard.blocking_recv() {
            if let AppEvent::HostKeyAsked(ask) = event {
                counted.fetch_add(1, Ordering::SeqCst);
                assert_eq!(ask.question.host, "h.example");
                assert_eq!(ask.question.key_type, "ED25519");
                answering.answer_host_key(ask.id, accept);
            }
        }
    });
    (state, asks)
}

fn remote_asking(prompt: &str) -> test_fixtures::Fixture {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&["remote", "set-url", "origin", "ssh://git@h.example/x.git"])
        .unwrap();
    let prompt_file = f.path().join(".git").join("prompt.txt");
    std::fs::write(&prompt_file, prompt).unwrap();
    let script = f.path().join(".git").join("fake-ssh.sh");
    let prompt_path = prompt_file.to_string_lossy().replace('\\', "/");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\n[ \"$1\" = -G ] && exit 0\nanswer=$(\"$SSH_ASKPASS\" \"$(cat '{prompt_path}')\")\necho \"answer=[$answer]\" >&2\nexit 255\n"
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    f.git(&[
        "config",
        "core.sshCommand",
        &script.to_string_lossy().replace('\\', "/"),
    ])
    .unwrap();
    f
}

fn ssh_answer(prompt: &str) -> String {
    let f = remote_asking(prompt);
    let repo = RepoHandle::open(f.path()).unwrap();
    answer_in(repo.fetch("origin", |_| None, |_| {}))
}

fn answer_in(fetched: Result<(), GitError>) -> String {
    match fetched {
        Err(GitError::Command(failure)) => failure
            .stderr
            .lines()
            .find_map(|line| line.strip_prefix("answer="))
            .unwrap_or_else(|| panic!("{}", failure.stderr))
            .to_owned(),
        other => panic!("{other:?}"),
    }
}

fn a_new_host_key_is_accepted_only_by_the_user() {
    let (_state, asks) = state_answering(true);
    assert_eq!(ssh_answer(NEW_HOST), "[yes]");
    assert_eq!(asks.load(Ordering::SeqCst), 1);
}

fn a_declined_host_key_answers_no() {
    let (_state, asks) = state_answering(false);
    assert_eq!(ssh_answer(NEW_HOST), "[]");
    assert_eq!(asks.load(Ordering::SeqCst), 1);
}

fn a_changed_key_and_other_questions_are_refused_unasked() {
    let (_state, asks) = state_answering(true);
    assert_eq!(ssh_answer(CHANGED), "[]");
    assert_eq!(ssh_answer(PASSWORD), "[]");
    assert_eq!(asks.load(Ordering::SeqCst), 0);
}

fn a_repository_in_the_agent_asks_the_window() {
    let (state, asks) = window_answering(true);
    let f = remote_asking(NEW_HOST);
    let agent = app_state::Host::Loopback(env!("CARGO_BIN_EXE_cogit-agent").into());
    let repo = state.open_repository_on(&agent, f.path()).unwrap().repo;
    let fetched = state.fetch(repo, "origin", &NetworkStop::default(), None, |_| {});
    assert_eq!(answer_in(fetched), "[yes]");
    assert_eq!(asks.load(Ordering::SeqCst), 1);
}

fn stopping_an_agent_fetch_withdraws_the_host_key_question() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    runtime.block_on(async {
        let state = Arc::new(AppState::new());
        let mut heard = state.subscribe();
        let f = remote_asking(NEW_HOST);
        let agent = app_state::Host::Loopback(env!("CARGO_BIN_EXE_cogit-agent").into());
        let repo = state.open_repository_on(&agent, f.path()).unwrap().repo;
        let permit = state.enqueue(repo, OperationKind::Fetch, "Fetching").await;
        let running = state.network_stop(permit.id());
        let (completed, result) = std::sync::mpsc::channel();
        {
            let (state, stop) = (Arc::clone(&state), running.token());
            std::thread::spawn(move || {
                let _ = completed.send(state.fetch(repo, "origin", &stop, None, |_| {}));
            });
        }

        let id = tokio::time::timeout(test_fixtures::scaled(Duration::from_secs(15)), async {
            loop {
                if let AppEvent::HostKeyAsked(ask) = heard.recv().await.unwrap() {
                    break ask.id;
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("the agent never asked the window"));

        assert!(state.cancel_network(permit.id()));
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let AppEvent::HostKeyWithdrawn { id: withdrawn } = heard.recv().await.unwrap() {
                    assert_eq!(withdrawn, id);
                    break;
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("Stop did not withdraw the host key question"));
        assert!(state.host_key_questions().is_empty());
        let fetched = result.recv_timeout(Duration::from_secs(10)).unwrap();
        permit.finish(fetched.is_ok());
        assert!(
            matches!(fetched, Err(GitError::Cancelled(_))),
            "{fetched:?}"
        );
    });
}

fn main() {
    if let Some(code) = git_engine::run_as_helper() {
        std::process::exit(code);
    }
    let tests: [(&str, fn()); 5] = [
        (
            "a_repository_in_the_agent_asks_the_window",
            a_repository_in_the_agent_asks_the_window,
        ),
        (
            "stopping_an_agent_fetch_withdraws_the_host_key_question",
            stopping_an_agent_fetch_withdraws_the_host_key_question,
        ),
        (
            "a_new_host_key_is_accepted_only_by_the_user",
            a_new_host_key_is_accepted_only_by_the_user,
        ),
        (
            "a_declined_host_key_answers_no",
            a_declined_host_key_answers_no,
        ),
        (
            "a_changed_key_and_other_questions_are_refused_unasked",
            a_changed_key_and_other_questions_are_refused_unasked,
        ),
    ];
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--list") {
        if !args.iter().any(|a| a == "--ignored") {
            for (name, _) in tests {
                println!("{name}: test");
            }
        }
        return;
    }
    let wanted: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();
    for (name, test) in tests {
        if wanted.is_empty() || wanted.iter().any(|w| name.contains(w.as_str())) {
            test();
            println!("{name} ... ok");
        }
    }
}
