//! F-082 end to end: a server that wants Basic auth, git without any way to ask, and the
//! Sign in dialog's retry, Remember and Remove as the backend sees them. The sign-in reaches
//! git through Cogit's credential helper (R-891), which is the running executable: this
//! test is its own `main` (no libtest harness) so that, started by git as the helper, it
//! answers as Cogit does.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use app_state::{AppState, SignIn};
use git_engine::{GitError, NetworkStop};
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Stdio;

const USER: &str = "ann";
const PASSWORD: &str = "pw";

/// `git http-backend` behind Basic auth for `USER:PASSWORD`, serving the folder `root`.
fn serve(root: PathBuf) -> u16 {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = server.local_addr().unwrap().port();
    // base64("ann:pw")
    let wanted = "Basic YW5uOnB3".to_owned();
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
    let mut cgi = test_fixtures::git_command_in(root)
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
fn repository() -> (
    test_fixtures::Fixture,
    tempfile::TempDir,
    AppState,
    app_state::RepoId,
) {
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
    let store = tempfile::tempdir().unwrap();
    let state = AppState::new();
    state.use_password_store(store.path().join("passwords.json"));
    state.create_credential_store(Some("master")).unwrap();
    let repo = state.open_repository(f.path()).unwrap().repo;
    (f, store, state, repo)
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
        key_path: None,
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

fn a_refusal_asks_a_retry_signs_in_and_remember_keeps_it() {
    let (_f, _store, state, repo) = repository();

    let asked = auth_required(fetch(&state, repo, None));
    assert_eq!(asked.host, "127.0.0.1");
    assert_eq!(asked.username, None);

    fetch(&state, repo, Some(&sign_in(PASSWORD, true))).unwrap();
    let known = state.credential_store().unwrap().known;
    assert_eq!(known.len(), 1);
    assert_eq!(known[0].username.as_deref(), Some(USER));

    fetch(&state, repo, None).unwrap();
}

fn a_wrong_password_asks_again_with_its_username_and_is_not_kept() {
    let (_f, _store, state, repo) = repository();

    let asked = auth_required(fetch(&state, repo, Some(&sign_in("wrong", true))));

    assert_eq!(asked.username.as_deref(), Some(USER));
    assert!(state.credential_store().unwrap().known.is_empty());
}

fn without_remember_the_retry_works_once_and_the_next_fetch_asks() {
    let (_f, _store, state, repo) = repository();

    fetch(&state, repo, Some(&sign_in(PASSWORD, false))).unwrap();

    auth_required(fetch(&state, repo, None));
}

fn a_removed_sign_in_is_asked_for_again() {
    let (_f, _store, state, repo) = repository();
    fetch(&state, repo, Some(&sign_in(PASSWORD, true))).unwrap();

    state.forget_credential("127.0.0.1", false).unwrap();

    auth_required(fetch(&state, repo, None));
}

// A sign-in typed for one host must never reach another, here the push URL's.
fn a_sign_in_goes_only_to_its_host() {
    let (f, _store, state, repo) = repository();
    let elsewhere = SignIn {
        host: "git.example".to_owned(),
        ..sign_in(PASSWORD, true)
    };

    let asked = auth_required(fetch(&state, repo, Some(&elsewhere)));

    assert_eq!(asked.host, "127.0.0.1");
    assert!(state.credential_store().unwrap().known.is_empty());
    drop(f);
}

fn main() {
    if let Some(code) = git_engine::run_as_helper() {
        std::process::exit(code);
    }
    let tests: [(&str, fn()); 5] = [
        (
            "a_refusal_asks_a_retry_signs_in_and_remember_keeps_it",
            a_refusal_asks_a_retry_signs_in_and_remember_keeps_it,
        ),
        (
            "a_wrong_password_asks_again_with_its_username_and_is_not_kept",
            a_wrong_password_asks_again_with_its_username_and_is_not_kept,
        ),
        (
            "without_remember_the_retry_works_once_and_the_next_fetch_asks",
            without_remember_the_retry_works_once_and_the_next_fetch_asks,
        ),
        (
            "a_removed_sign_in_is_asked_for_again",
            a_removed_sign_in_is_asked_for_again,
        ),
        (
            "a_sign_in_goes_only_to_its_host",
            a_sign_in_goes_only_to_its_host,
        ),
    ];
    let args: Vec<String> = std::env::args().skip(1).collect();
    // nextest lists in libtest's terse format, then runs one name at a time.
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
