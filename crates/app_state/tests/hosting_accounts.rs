#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, HostingError, HostingProvider, NewHostingAccount, StoreState};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

fn serve(replies: Vec<(u16, &'static str)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for (status, body) in replies {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line.trim().is_empty() {
                    break;
                }
            }
            let mut stream = stream;
            write!(
                stream,
                "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });
    base
}

fn state_with_store() -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new();
    state.use_password_store(dir.path().join("passwords.json"));
    state.create_credential_store(None).unwrap();
    (dir, state)
}

fn enterprise(server: &str, token: &str, use_for_git: bool) -> NewHostingAccount {
    NewHostingAccount {
        provider: HostingProvider::Github,
        server_url: Some(server.to_owned()),
        username: None,
        token: token.to_owned(),
        use_for_git,
    }
}

#[test]
fn an_added_account_keeps_its_token_out_of_everything_the_page_reads() {
    let (dir, state) = state_with_store();
    let server = serve(vec![(200, r#"{"login":"ann"}"#)]);
    let added = state
        .add_hosting_account(&enterprise(&server, " ghp_secret ", true))
        .unwrap();
    assert_eq!(added.login, "ann");

    let listed = state.hosting_accounts().unwrap();
    assert_eq!(listed.state, StoreState::Unlocked);
    assert_eq!(listed.accounts, vec![added.clone()]);
    let json = serde_json::to_string(&listed).unwrap();
    assert!(!json.contains("ghp_secret"), "{json}");
    assert!(state.credential_store().unwrap().known.is_empty());

    let file = std::fs::read_to_string(dir.path().join("passwords.json")).unwrap();
    assert!(!file.contains("ghp_secret"));
    assert_eq!(&*state.hosting_token(&added.id).unwrap(), "ghp_secret");

    let reopened = AppState::new();
    reopened.use_password_store(dir.path().join("passwords.json"));
    assert_eq!(reopened.hosting_accounts().unwrap().accounts, vec![added]);
}

#[test]
fn a_refused_token_is_not_kept() {
    let (_dir, state) = state_with_store();
    let server = serve(vec![(401, r#"{"message":"Bad credentials"}"#)]);
    let error = state
        .add_hosting_account(&enterprise(&server, "bad", true))
        .unwrap_err();
    assert!(matches!(error, HostingError::Api(_)), "{error:?}");
    assert!(state.hosting_accounts().unwrap().accounts.is_empty());
}

#[test]
fn the_token_signs_git_in_over_https_for_its_repositories_only() {
    let (_dir, state) = state_with_store();
    let server = serve(vec![(200, r#"{"login":"ann"}"#)]);
    state
        .add_hosting_account(&enterprise(&server, "ghp_git", true))
        .unwrap();
    let remote = format!("{server}/acme/app.git");
    let credential = state.stored_credential(&remote).unwrap();
    assert_eq!(credential.username.as_deref(), Some("ann"));
    assert_eq!(credential.secret, "ghp_git");

    let matched = state.hosting_account_for(&remote).unwrap().unwrap();
    assert_eq!(matched.repository, "acme/app");
    assert_eq!(matched.api_url, format!("{server}/api/v3"));
    assert!(
        state
            .stored_credential("https://other.example/a.git")
            .is_none()
    );
}

#[test]
fn a_sign_in_of_the_host_wins_and_an_account_kept_from_git_is_not_offered() {
    let (_dir, state) = state_with_store();
    let server = serve(vec![(200, r#"{"login":"ann"}"#)]);
    state
        .add_hosting_account(&enterprise(&server, "ghp_api", false))
        .unwrap();
    let remote = format!("{server}/acme/app.git");
    assert!(state.stored_credential(&remote).is_none());
    assert!(state.hosting_account_for(&remote).unwrap().is_some());
}

#[test]
fn remove_all_sign_ins_keeps_accounts_and_remove_drops_one() {
    let (_dir, state) = state_with_store();
    let server = serve(vec![
        (200, r#"{"login":"ann"}"#),
        (200, r#"{"login":"ann2"}"#),
    ]);
    let added = state
        .add_hosting_account(&enterprise(&server, "t", true))
        .unwrap();
    state.forget_all_credentials().unwrap();
    assert_eq!(state.hosting_accounts().unwrap().accounts.len(), 1);

    assert_eq!(state.check_hosting_account(&added.id).unwrap(), "ann2");
    assert_eq!(state.hosting_accounts().unwrap().accounts[0].login, "ann2");

    state.remove_hosting_account(&added.id).unwrap();
    assert!(state.hosting_accounts().unwrap().accounts.is_empty());
    assert!(matches!(
        state.check_hosting_account(&added.id),
        Err(HostingError::NotFound)
    ));
}

#[test]
fn adding_the_same_login_again_replaces_its_token() {
    let (_dir, state) = state_with_store();
    let server = serve(vec![
        (200, r#"{"login":"ann"}"#),
        (200, r#"{"login":"ann"}"#),
    ]);
    let first = state
        .add_hosting_account(&enterprise(&server, "old", true))
        .unwrap();
    let second = state
        .add_hosting_account(&enterprise(&server, "new", true))
        .unwrap();
    assert_eq!(first.id, second.id);
    assert_eq!(state.hosting_accounts().unwrap().accounts.len(), 1);
    assert_eq!(&*state.hosting_token(&first.id).unwrap(), "new");
}

#[test]
fn a_locked_store_lists_no_accounts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("passwords.json");
    let first = AppState::new();
    first.use_password_store(path.clone());
    first.create_credential_store(Some("master")).unwrap();
    let state = AppState::new();
    state.use_password_store(path);
    let listed = state.hosting_accounts().unwrap();
    assert_eq!(listed.state, StoreState::Locked);
    assert!(listed.accounts.is_empty());
    assert!(
        state
            .add_hosting_account(&enterprise("http://127.0.0.1:9", "t", true))
            .is_err()
    );
}
