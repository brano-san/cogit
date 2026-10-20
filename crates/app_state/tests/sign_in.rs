//! F-082 end to end: a server that wants Basic auth, git without any way to ask, and the
//! Sign in dialog's retry, Remember and Remove as the backend sees them.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, MemoryStore, SignIn};
use git_engine::{GitError, NetworkStop};
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const USER: &str = "ann";
const PASSWORD: &str = "pw";

/// `git http-backend` behind Basic auth for `USER:PASSWORD`, serving the folder `root`.
fn serve(root: PathBuf) -> u16 {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = server.local_addr().unwrap().port();
    let wanted = git_engine::basic_header(&git_engine::Credential {
        username: Some(USER.to_owned()),
        secret: PASSWORD.to_owned(),
    });
    let wanted = wanted.trim_start_matches("Authorization: ").to_owned();
    std::thread::spawn(move || {
        for socket in server.incoming().flatten() {
            let (root, wanted) = (root.clone(), wanted.clone());
            std::thread::spawn(move || answer(socket, &root, &wanted));
        }
    });
    port
}

fn answer(mut socket: TcpStream, root: &Path, wanted: &str) {
    let mut reader = BufReader::new(socket.try_clone().unwrap());
    let mut request = String::new();
    reader.read_line(&mut request).unwrap();
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_lowercase(), value.trim().to_owned()));
        }
    }
    let header = |name: &str| {
        headers
            .iter()
            .find(|(n, _)| n == name)
            .map_or("", |(_, v)| v.as_str())
            .to_owned()
    };
    let mut body = vec![0; header("content-length").parse().unwrap_or(0)];
    reader.read_exact(&mut body).unwrap();

    if header("authorization") != wanted {
        let _ = socket.write_all(
            b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"test\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        );
        return;
    }
    let mut parts = request.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let target = parts.next().unwrap_or("/");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let mut cgi = Command::new("git")
        .arg("http-backend")
        .env("GIT_PROJECT_ROOT", root)
        .env("GIT_HTTP_EXPORT_ALL", "1")
        .env("REQUEST_METHOD", method)
        .env("PATH_INFO", path)
        .env("QUERY_STRING", query)
        .env("CONTENT_TYPE", header("content-type"))
        .env("CONTENT_LENGTH", body.len().to_string())
        .env("HTTP_CONTENT_ENCODING", header("content-encoding"))
        .env("GIT_PROTOCOL", header("git-protocol"))
        .env("REMOTE_USER", USER)
        .env("REMOTE_ADDR", "127.0.0.1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    cgi.stdin.take().unwrap().write_all(&body).unwrap();
    let out = cgi.wait_with_output().unwrap().stdout;
    let split = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let (cgi_headers, content) = (String::from_utf8_lossy(&out[..split]), &out[split + 4..]);
    let mut status = "200 OK".to_owned();
    let mut response = String::new();
    for line in cgi_headers.lines() {
        match line.strip_prefix("Status: ") {
            Some(given) => status = given.to_owned(),
            None => response.push_str(&format!("{line}\r\n")),
        }
    }
    let head = format!(
        "HTTP/1.1 {status}\r\n{response}Content-Length: {}\r\nConnection: close\r\n\r\n",
        content.len()
    );
    let _ = socket.write_all(head.as_bytes());
    let _ = socket.write_all(content);
}

/// A clone whose origin is served over HTTP behind Basic auth. No credential helper: the
/// developer's own (Git Credential Manager) would open its window in the middle of a test.
fn repository() -> (test_fixtures::Fixture, AppState, app_state::RepoId) {
    let f = test_fixtures::with_remote().unwrap();
    let origin = PathBuf::from(f.git(&["remote", "get-url", "origin"]).unwrap().trim());
    let port = serve(origin.parent().unwrap().to_path_buf());
    let name = origin.file_name().unwrap().to_string_lossy().into_owned();
    f.git(&[
        "remote",
        "set-url",
        "origin",
        &format!("http://127.0.0.1:{port}/{name}"),
    ])
    .unwrap();
    f.git(&["config", "credential.helper", ""]).unwrap();
    let state = AppState::new().with_secrets(Box::new(MemoryStore::default()));
    let repo = state.open_repository(f.path()).unwrap().repo;
    (f, state, repo)
}

fn fetch(
    state: &AppState,
    repo: app_state::RepoId,
    sign_in: Option<&SignIn>,
) -> Result<(), GitError> {
    state.fetch(repo, "origin", &NetworkStop::default(), sign_in, |_| {})
}

fn sign_in(password: &str, remember: bool) -> SignIn {
    SignIn {
        host: "127.0.0.1".to_owned(),
        username: Some(USER.to_owned()),
        secret: password.to_owned(),
        remember,
    }
}

fn auth_required(result: Result<(), GitError>) -> git_engine::AuthRequired {
    match result {
        Err(GitError::Command(failure)) => failure
            .auth_required
            .unwrap_or_else(|| panic!("not an auth failure: {}", failure.stderr)),
        other => panic!("expected a refusal: {other:?}"),
    }
}

#[test]
fn a_refusal_asks_a_retry_signs_in_and_remember_keeps_it() {
    let (_f, state, repo) = repository();

    let asked = auth_required(fetch(&state, repo, None));
    assert_eq!(asked.host, "127.0.0.1");
    assert_eq!(asked.username, None);

    fetch(&state, repo, Some(&sign_in(PASSWORD, true))).unwrap();
    let known = state.list_credentials().known;
    assert_eq!(known.len(), 1);
    assert_eq!(known[0].username.as_deref(), Some(USER));

    fetch(&state, repo, None).unwrap();
}

#[test]
fn a_wrong_password_asks_again_with_its_username_and_is_not_kept() {
    let (_f, state, repo) = repository();

    let asked = auth_required(fetch(&state, repo, Some(&sign_in("wrong", true))));

    assert_eq!(asked.username.as_deref(), Some(USER));
    assert!(state.list_credentials().known.is_empty());
}

#[test]
fn without_remember_the_retry_works_once_and_the_next_fetch_asks() {
    let (_f, state, repo) = repository();

    fetch(&state, repo, Some(&sign_in(PASSWORD, false))).unwrap();

    auth_required(fetch(&state, repo, None));
}

#[test]
fn a_removed_sign_in_is_asked_for_again() {
    let (_f, state, repo) = repository();
    fetch(&state, repo, Some(&sign_in(PASSWORD, true))).unwrap();

    state.forget_credential("127.0.0.1").unwrap();

    auth_required(fetch(&state, repo, None));
}

// A sign-in typed for one host must never reach another, here the push URL's.
#[test]
fn a_sign_in_goes_only_to_its_host() {
    let (f, state, repo) = repository();
    let elsewhere = SignIn {
        host: "git.example".to_owned(),
        ..sign_in(PASSWORD, true)
    };

    let asked = auth_required(fetch(&state, repo, Some(&elsewhere)));

    assert_eq!(asked.host, "127.0.0.1");
    assert!(state.list_credentials().known.is_empty());
    drop(f);
}
