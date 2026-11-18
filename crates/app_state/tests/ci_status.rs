#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, CiState, CiTarget, HostingProvider, NewHostingAccount, RepoId};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;

fn serve(replies: Vec<(u16, &'static str, String)>) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (send, seen) = mpsc::channel();
    std::thread::spawn(move || {
        let mut replies = replies.into_iter();
        while let Ok((stream, _)) = listener.accept() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line.trim().is_empty() {
                    break;
                }
            }
            let path = first.split_whitespace().nth(1).unwrap_or("").to_owned();
            let (status, headers, body) =
                replies
                    .next()
                    .unwrap_or((500, "", r#"{"message":"unexpected"}"#.to_owned()));
            let mut stream = stream;
            write!(
                stream,
                "HTTP/1.1 {status} X\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            if send.send(path).is_err() {
                break;
            }
        }
    });
    (base, seen)
}

fn repo_at(server: &str) -> (test_fixtures::Fixture, String) {
    let f = test_fixtures::with_remote().unwrap();
    f.git(&[
        "remote",
        "set-url",
        "origin",
        &format!("{server}/acme/app.git"),
    ])
    .unwrap();
    let head = f.git(&["rev-parse", "HEAD"]).unwrap().trim().to_owned();
    (f, head)
}

fn account(server: &str) -> NewHostingAccount {
    NewHostingAccount {
        provider: HostingProvider::Github,
        server_url: Some(server.to_owned()),
        username: None,
        token: "ghp_ci".to_owned(),
        use_for_git: false,
    }
}

fn target(sha: &str) -> Vec<CiTarget> {
    vec![CiTarget {
        sha: sha.to_owned(),
        remote: None,
    }]
}

fn open(state: &AppState, f: &test_fixtures::Fixture) -> RepoId {
    state.open_repository(f.path()).unwrap().repo
}

#[test]
fn a_commit_on_origin_gets_its_checks_once_and_then_from_the_cache() {
    let finished = r#"{"state":"success","statuses":[{"context":"ci/build","state":"success","target_url":"https://ci/1"}]}"#;
    let runs = r#"{"total_count":1,"check_runs":[{"name":"test","status":"completed","conclusion":"failure","started_at":"2026-01-01T00:00:00Z","completed_at":"2026-01-01T00:01:00Z","html_url":"https://gh/runs/1"}]}"#;
    let (server, seen) = serve(vec![
        (200, "", r#"{"login":"ann"}"#.to_owned()),
        (200, "X-RateLimit-Remaining: 4999\r\n", finished.to_owned()),
        (200, "X-RateLimit-Remaining: 4998\r\n", runs.to_owned()),
    ]);
    let (f, head) = repo_at(&server);
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new();
    state.use_password_store(dir.path().join("passwords.json"));
    state.create_credential_store(None).unwrap();
    state.add_hosting_account(&account(&server)).unwrap();
    assert_eq!(seen.recv().unwrap(), "/api/v3/user");
    let repo = open(&state, &f);

    let batch = state.ci_statuses(repo, &target(&head)).unwrap();
    assert_eq!(batch.statuses.len(), 1);
    let status = &batch.statuses[0];
    assert_eq!(status.sha, head);
    assert_eq!(status.state, CiState::Failure);
    assert_eq!(status.checks.len(), 2);
    assert_eq!(status.checks[0].name, "test");
    assert_eq!(status.checks[0].duration_secs, Some(60));
    assert_eq!(
        status.url.as_deref(),
        Some(format!("{server}/acme/app/commit/{head}").as_str())
    );
    assert_eq!(batch.retry_ms, None, "a finished status is not polled");
    assert_eq!(
        seen.recv().unwrap(),
        format!("/api/v3/repos/acme/app/commits/{head}/status?per_page=100")
    );
    assert!(seen.recv().unwrap().contains("/check-runs"));

    let again = state.ci_statuses(repo, &target(&head)).unwrap();
    assert_eq!(again.statuses[0].state, CiState::Failure);
    assert!(seen.recv_timeout(Duration::from_millis(300)).is_err());
}

#[test]
fn a_rate_limit_stops_asking_and_says_why() {
    let pending = r#"{"state":"pending","statuses":[{"context":"ci","state":"pending"}]}"#;
    let (server, seen) = serve(vec![
        (200, "", r#"{"login":"ann"}"#.to_owned()),
        (200, "", pending.to_owned()),
        (200, "", r#"{"total_count":0,"check_runs":[]}"#.to_owned()),
        (
            429,
            "Retry-After: 120\r\n",
            r#"{"message":"secondary rate limit"}"#.to_owned(),
        ),
    ]);
    let (f, head) = repo_at(&server);
    f.commit_file(2, "two.txt", "2\n").unwrap();
    let second = f.git(&["rev-parse", "HEAD"]).unwrap().trim().to_owned();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new();
    state.use_password_store(dir.path().join("passwords.json"));
    state.create_credential_store(None).unwrap();
    state.add_hosting_account(&account(&server)).unwrap();
    let repo = open(&state, &f);

    let batch = state.ci_statuses(repo, &target(&head)).unwrap();
    assert_eq!(batch.statuses[0].state, CiState::Pending);
    let retry = batch.retry_ms.unwrap();
    assert!((14_000..=15_000).contains(&retry), "{retry}");
    for _ in 0..3 {
        seen.recv().unwrap();
    }

    let limited = state.ci_statuses(repo, &target(&second)).unwrap();
    assert!(limited.statuses.is_empty());
    assert!(limited.retry_ms.unwrap() > 100_000);
    seen.recv().unwrap();

    let both = [target(&head), target(&second)].concat();
    let held = state.ci_statuses(repo, &both).unwrap();
    assert_eq!(held.statuses.len(), 1);
    assert!(
        held.statuses[0]
            .note
            .as_deref()
            .unwrap()
            .contains("rate limit"),
        "{:?}",
        held.statuses[0].note
    );
    assert!(seen.recv_timeout(Duration::from_millis(300)).is_err());
}

#[test]
fn a_locked_store_or_no_account_asks_nobody() {
    let (server, seen) = serve(vec![(200, "", r#"{"login":"ann"}"#.to_owned())]);
    let (f, head) = repo_at(&server);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("passwords.json");

    let empty = AppState::new();
    empty.use_password_store(path.clone());
    empty.create_credential_store(Some("secret")).unwrap();
    let repo = open(&empty, &f);
    let none = empty.ci_statuses(repo, &target(&head)).unwrap();
    assert!(none.statuses.is_empty());
    assert_eq!(none.retry_ms, None);
    empty.add_hosting_account(&account(&server)).unwrap();
    seen.recv().unwrap();

    let locked = AppState::new();
    locked.use_password_store(path);
    let repo = open(&locked, &f);
    let batch = locked.ci_statuses(repo, &target(&head)).unwrap();
    assert!(batch.statuses.is_empty());
    assert_eq!(batch.retry_ms, None);
    assert!(seen.recv_timeout(Duration::from_millis(300)).is_err());
}
