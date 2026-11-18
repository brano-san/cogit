#![allow(clippy::unwrap_used, clippy::expect_used)]

use hosting::{ApiError, HostingAccount, HostingProvider, check, check_at};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc;

struct Seen {
    path: String,
    authorization: String,
}

fn serve(status: u16, body: &'static str) -> (String, mpsc::Receiver<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (send, seen) = mpsc::channel();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let path = line.split_whitespace().nth(1).unwrap_or("").to_owned();
        let mut authorization = String::new();
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header.trim().is_empty() {
                break;
            }
            if let Some((name, value)) = header.split_once(':')
                && name.eq_ignore_ascii_case("authorization")
            {
                authorization = value.trim().to_owned();
            }
        }
        let mut stream = stream;
        write!(
            stream,
            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        send.send(Seen {
            path,
            authorization,
        })
        .unwrap();
    });
    (base, seen)
}

fn account(provider: HostingProvider, server: &str) -> HostingAccount {
    HostingAccount {
        id: "a".into(),
        provider,
        server_url: Some(server.to_owned()),
        login: String::new(),
        username: None,
        use_for_git: true,
    }
}

#[test]
fn github_enterprise_answers_with_the_login() {
    let (base, seen) = serve(200, r#"{"login":"ann","id":1}"#);
    let login = check(
        &account(HostingProvider::Github, &base),
        "ghp_abc",
        Some("http://proxy.invalid:1"),
    )
    .unwrap();
    assert_eq!(login, "ann");
    let seen = seen.recv().unwrap();
    assert_eq!(seen.path, "/api/v3/user");
    assert_eq!(seen.authorization, "Bearer ghp_abc");
}

#[test]
fn self_hosted_gitlab_under_a_path() {
    let (base, seen) = serve(200, r#"{"username":"bob"}"#);
    let server = format!("{base}/gitlab");
    let login = check(&account(HostingProvider::Gitlab, &server), "glpat", None).unwrap();
    assert_eq!(login, "bob");
    assert_eq!(seen.recv().unwrap().path, "/gitlab/api/v4/user");
}

#[test]
fn bitbucket_signs_in_with_email_and_token() {
    let (base, seen) = serve(200, r#"{"username":"carol","display_name":"Carol"}"#);
    let api = format!("{base}/2.0");
    let login = check_at(
        &api,
        HostingProvider::Bitbucket,
        Some("c@x.io"),
        "tok",
        None,
    )
    .unwrap();
    assert_eq!(login, "carol");
    let seen = seen.recv().unwrap();
    assert_eq!(seen.path, "/2.0/user");
    assert_eq!(seen.authorization, "Basic Y0B4LmlvOnRvaw==");
}

#[test]
fn a_refused_token_says_so_without_the_token() {
    let (base, _seen) = serve(401, r#"{"message":"Bad credentials"}"#);
    let error = check(&account(HostingProvider::Github, &base), "ghp_secret", None).unwrap_err();
    assert!(matches!(error, ApiError::Refused(_, 401)), "{error:?}");
    assert!(!error.to_string().contains("ghp_secret"));
}

#[test]
fn an_unexpected_reply_is_an_error() {
    let (base, _seen) = serve(200, r#"{"id":1}"#);
    let error = check(&account(HostingProvider::Gitlab, &base), "t", None).unwrap_err();
    assert!(matches!(error, ApiError::Reply(_)), "{error:?}");
}

#[test]
fn nobody_listening_is_a_network_error() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let error = check(&account(HostingProvider::Github, &base), "t", None).unwrap_err();
    assert!(matches!(error, ApiError::Network(..)), "{error:?}");
}
