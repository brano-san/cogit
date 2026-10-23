// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{
    AppState, SignIn, StoreState, credential_host, credential_target, host_of, ssh_host,
};

#[test]
fn the_host_of_an_https_remote_is_its_domain() {
    assert_eq!(
        host_of("https://github.com/owner/repo.git").as_deref(),
        Some("github.com")
    );
}

#[test]
fn the_host_of_an_scp_style_ssh_remote_is_its_domain() {
    assert_eq!(
        host_of("git@github.com:owner/repo.git").as_deref(),
        Some("github.com")
    );
}

#[test]
fn the_host_of_an_https_remote_drops_an_embedded_user() {
    assert_eq!(
        host_of("https://me@gitlab.com/o/r.git").as_deref(),
        Some("gitlab.com")
    );
}

#[test]
fn a_port_is_not_part_of_the_host() {
    assert_eq!(
        host_of("https://example.com:8443/o/r.git").as_deref(),
        Some("example.com")
    );
}

#[test]
fn a_local_path_has_no_host() {
    assert!(host_of("/srv/git/repo.git").is_none());
    assert!(host_of("C:/work/repo").is_none());
}

#[test]
fn a_stored_token_is_for_https_only() {
    assert_eq!(credential_host("http://git.corp.example/r.git"), None);
    assert_eq!(
        credential_host("https://git.corp.example/r.git").as_deref(),
        Some("git.corp.example")
    );
}

#[test]
fn a_host_without_a_dot_has_a_credential_host() {
    assert_eq!(
        credential_host("https://gitlab/x.git").as_deref(),
        Some("gitlab")
    );
    assert_eq!(
        credential_host("https://tfs:8080/x").as_deref(),
        Some("tfs")
    );
    assert_eq!(
        credential_host("https://a@b@host/x").as_deref(),
        Some("host")
    );
    assert_eq!(credential_host("git@github.com:o/r.git"), None);
    assert_eq!(credential_host("ssh://git@github.com/o/r.git"), None);
}

// The frontend used to keep its own copy of this rule and the two disagreed here: a token
// saved under `evil` was never the one read for `h`.
#[test]
fn the_edges_of_a_url_have_one_answer() {
    let host = |url: &str| credential_host(url);
    assert_eq!(host("https://h?a@evil/x").as_deref(), Some("h"));
    assert_eq!(host("https://h?x").as_deref(), Some("h"));
    assert_eq!(host("https://h#a@evil").as_deref(), Some("h"));
    assert_eq!(
        host("https://u:p%40ss@Git.Example.COM:8443/r").as_deref(),
        Some("git.example.com")
    );
    assert_eq!(host("HTTPS://example.com/r"), None);
    assert_eq!(
        host("https://git.пример.рф/r").as_deref(),
        Some("git.пример.рф")
    );
    assert_eq!(host("https://[::1]:8443/r").as_deref(), Some("[::1]"));
    assert_eq!(host("https://[::1/r"), None);
    assert_eq!(host("https:///r"), None);
    assert_eq!(host("https://"), None);
    assert_eq!(host("https:/x"), None);
    assert_eq!(host("htt"), None);
    assert_eq!(host("https://é").as_deref(), Some("é"));
}

fn state_with_store() -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new();
    state.use_password_store(dir.path().join("passwords.json"));
    state.create_credential_store(None).unwrap();
    (dir, state)
}

// Save through the door the Authentication page uses, read through the one network runs
// use: both must land on the same key, whatever the URL looks like.
#[test]
fn a_sign_in_saved_for_a_url_is_the_one_a_network_run_reads() {
    let (_dir, state) = state_with_store();
    for url in [
        "https://github.com/o/r.git",
        "https://me@gitlab.com:8443/o/r.git",
        "https://h?a@evil/x",
        "https://gitlab/g/x.git",
        "https://git.пример.рф/r.git",
        "https://[::1]:8443/r.git",
    ] {
        state.store_credential(&sign_in(url, None, "tok")).unwrap();
        assert_eq!(
            state.stored_credential(url).map(|c| c.secret).as_deref(),
            Some("tok"),
            "{url}"
        );
        state.forget_credential(url, false).unwrap();
        assert_eq!(state.stored_credential(url), None, "{url}");
    }
}

#[test]
fn a_login_never_goes_over_plain_http_or_ssh() {
    let (_dir, state) = state_with_store();
    state
        .store_credential(&sign_in("github.com", None, "x"))
        .unwrap();
    for url in [
        "http://github.com/o/r.git",
        "git@github.com:o/r.git",
        "/srv/r.git",
    ] {
        assert_eq!(state.stored_credential(url), None, "{url}");
    }
    assert!(
        state
            .store_credential(&sign_in("/srv/r.git", None, "x"))
            .is_err()
    );
    // The userinfo trick must not reach the sign-in of another host.
    state
        .store_credential(&sign_in("https://h/x", None, "h-token"))
        .unwrap();
    assert_eq!(
        state
            .stored_credential("https://h?a@evil/x")
            .map(|c| c.secret)
            .as_deref(),
        Some("h-token")
    );
    assert_eq!(state.stored_credential("https://evil/x"), None);
}

#[test]
fn an_ssh_key_is_kept_beside_a_login_of_the_same_host() {
    let (_dir, state) = state_with_store();
    state
        .store_credential(&sign_in("github.com", Some("ann"), "pw"))
        .unwrap();
    state
        .store_credential(&SignIn {
            key_path: Some("C:\\Users\\ann\\.ssh\\id_ed25519".to_owned()),
            ..sign_in("git@github.com:o/r.git", Some("ignored"), "")
        })
        .unwrap();

    let key = state
        .stored_credential("ssh://git@GitHub.com/o/r.git")
        .unwrap();
    assert_eq!(key.key.as_deref(), Some("C:\\Users\\ann\\.ssh\\id_ed25519"));
    assert_eq!((key.username, key.secret.as_str()), (None, ""));
    let login = state
        .stored_credential("https://github.com/o/r.git")
        .unwrap();
    assert_eq!((login.username.as_deref(), login.key), (Some("ann"), None));

    state.forget_credential("github.com", true).unwrap();
    assert!(state.stored_credential("git@github.com:o/r.git").is_none());
    assert!(state.stored_credential("https://github.com/x").is_some());
}

fn sign_in(host: &str, username: Option<&str>, secret: &str) -> SignIn {
    SignIn {
        host: host.to_owned(),
        username: username.map(str::to_owned),
        secret: secret.to_owned(),
        key_path: None,
        remember: true,
    }
}

#[test]
fn the_list_names_hosts_and_users_and_never_a_secret() {
    let (dir, state) = state_with_store();
    state
        .store_credential(&sign_in("b.example", Some("bob"), "secret-b"))
        .unwrap();
    state
        .store_credential(&sign_in("a.example", None, "secret-a"))
        .unwrap();

    let list = state.credential_store().unwrap();
    assert_eq!(list.state, StoreState::Unlocked);
    let shown: Vec<_> = list
        .known
        .iter()
        .map(|k| (k.host.as_str(), k.username.as_deref()))
        .collect();
    assert_eq!(shown, [("a.example", None), ("b.example", Some("bob"))]);
    let written = std::fs::read_to_string(dir.path().join("passwords.json")).unwrap();
    assert!(
        !written.contains("b.example") && !written.contains("secret"),
        "{written}"
    );

    let reopened = AppState::new();
    reopened.use_password_store(dir.path().join("passwords.json"));
    assert_eq!(reopened.credential_store().unwrap().known.len(), 2);
}

#[test]
fn forgetting_all_empties_the_store() {
    let (_dir, state) = state_with_store();
    state
        .store_credential(&sign_in("a.example", None, "x"))
        .unwrap();
    state
        .store_credential(&sign_in("b.example", None, "y"))
        .unwrap();

    state.forget_credential("a.example", false).unwrap();
    assert!(state.stored_credential("https://a.example/r").is_none());
    assert_eq!(state.credential_store().unwrap().known.len(), 1);

    state.forget_all_credentials().unwrap();
    assert!(state.stored_credential("https://b.example/r").is_none());
    assert!(state.credential_store().unwrap().known.is_empty());
}

// A store behind a master password holds nothing until it is unlocked: a background probe
// reads nothing and a Remember is refused, rather than either asking or failing loudly.
#[test]
fn a_locked_store_reads_nothing_and_keeps_nothing_until_unlocked() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("passwords.json");
    let first = AppState::new();
    first.use_password_store(path.clone());
    first.create_credential_store(Some("master")).unwrap();
    first
        .store_credential(&sign_in("a.example", None, "x"))
        .unwrap();

    let state = AppState::new();
    state.use_password_store(path);
    let locked = state.credential_store().unwrap();
    assert_eq!(
        (locked.state, locked.master_password),
        (StoreState::Locked, true)
    );
    assert!(locked.known.is_empty());
    assert!(state.stored_credential("https://a.example/r").is_none());
    assert!(
        state
            .store_credential(&sign_in("b.example", None, "y"))
            .is_err()
    );

    assert!(!state.unlock_credentials("wrong").unwrap());
    assert!(state.unlock_credentials("master").unwrap());
    assert_eq!(
        state
            .stored_credential("https://a.example/r")
            .map(|c| c.secret)
            .as_deref(),
        Some("x")
    );
}

#[test]
fn without_a_store_nothing_is_read_and_a_remember_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new();
    state.use_password_store(dir.path().join("passwords.json"));

    assert_eq!(state.credential_store().unwrap().state, StoreState::Missing);
    assert!(state.stored_credential("https://a.example/r").is_none());
    assert!(
        state
            .store_credential(&sign_in("a.example", None, "x"))
            .is_err()
    );
}

#[test]
fn plain_http_gets_a_sign_in_only_on_this_machine() {
    assert_eq!(
        credential_host("http://127.0.0.1:8080/r.git").as_deref(),
        Some("127.0.0.1")
    );
    assert_eq!(
        credential_host("http://localhost/r.git").as_deref(),
        Some("localhost")
    );
    assert_eq!(credential_host("http://git.example/r.git"), None);
    assert_eq!(
        app_state::host_input(" git.example:8443 ").as_deref(),
        Some("git.example")
    );
    assert_eq!(app_state::host_input("git example"), None);
}

#[test]
fn the_host_of_an_ssh_remote_is_found_in_every_spelling() {
    for url in [
        "git@GitHub.com:o/r.git",
        "ssh://git@github.com:2222/o/r.git",
        "git+ssh://github.com/o/r.git",
        "github.com:o/r.git",
    ] {
        assert_eq!(ssh_host(url).as_deref(), Some("github.com"), "{url}");
        assert_eq!(
            credential_target(url),
            Some(("github.com".to_owned(), true)),
            "{url}"
        );
    }
    for url in [
        "C:/work/repo",
        "C:\\work\\repo",
        "./a:b",
        "/srv/r.git",
        "https://github.com/o/r.git",
    ] {
        assert_eq!(ssh_host(url), None, "{url}");
    }
    assert_eq!(
        credential_target("https://github.com/o/r.git"),
        Some(("github.com".to_owned(), false))
    );
}

#[test]
fn only_private_keys_are_offered_the_standard_names_first() {
    let dir = tempfile::tempdir().unwrap();
    let write = |name: &str, text: &str| std::fs::write(dir.path().join(name), text).unwrap();
    let key = "-----BEGIN OPENSSH PRIVATE KEY-----\nb3Blbg==\n";
    write("id_rsa", "-----BEGIN RSA PRIVATE KEY-----\nx\n");
    write("id_ed25519", key);
    write("work_key", key);
    write("id_ed25519.pub", "ssh-ed25519 AAAA ann@host\n");
    write("known_hosts", "github.com ssh-ed25519 AAAA\n");
    write("config", "Host *\n  IdentityFile ~/.ssh/work_key\n");
    write("authorized_keys", "ssh-rsa AAAA\n");
    std::fs::create_dir(dir.path().join("id_ecdsa")).unwrap();

    let names: Vec<String> = app_state::private_keys(dir.path())
        .iter()
        .map(|path| {
            std::path::Path::new(path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();

    assert_eq!(names, ["id_ed25519", "id_rsa", "work_key"]);
    assert!(app_state::private_keys(&dir.path().join("missing")).is_empty());
}
