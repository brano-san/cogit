// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{AppState, MemoryStore, SecretStore, SignIn, credential_host, host_of};

#[test]
fn a_stored_secret_comes_back() {
    let store = MemoryStore::default();
    store.set("github.com", "ghp_token").unwrap();

    assert_eq!(store.get("github.com").as_deref(), Some("ghp_token"));
}

#[test]
fn an_unknown_key_has_no_secret() {
    assert!(MemoryStore::default().get("github.com").is_none());
}

#[test]
fn storing_twice_replaces_the_secret() {
    let store = MemoryStore::default();
    store.set("github.com", "first").unwrap();
    store.set("github.com", "second").unwrap();

    assert_eq!(store.get("github.com").as_deref(), Some("second"));
}

#[test]
fn deleting_removes_the_secret() {
    let store = MemoryStore::default();
    store.set("github.com", "token").unwrap();
    store.delete("github.com").unwrap();

    assert!(store.get("github.com").is_none());
}

#[test]
fn deleting_a_key_that_was_never_set_is_not_an_error() {
    assert!(MemoryStore::default().delete("nothing.example").is_ok());
}

#[test]
fn keys_do_not_leak_into_each_other() {
    let store = MemoryStore::default();
    store.set("github.com", "a").unwrap();
    store.set("gitlab.com", "b").unwrap();

    assert_eq!(store.get("github.com").as_deref(), Some("a"));
    assert_eq!(store.get("gitlab.com").as_deref(), Some("b"));
}

#[test]
fn an_empty_secret_is_refused_rather_than_stored() {
    assert!(MemoryStore::default().set("github.com", "").is_err());
}

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

// Save through the door the Authentication page uses, read through the one network runs
// use: both must land on the same key, whatever the URL looks like.
#[test]
fn a_sign_in_saved_for_a_url_is_the_one_a_network_run_reads() {
    let state = AppState::new().with_secrets(Box::new(MemoryStore::default()));
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
        state.forget_credential(url).unwrap();
        assert_eq!(state.stored_credential(url), None, "{url}");
    }
}

#[test]
fn a_sign_in_never_goes_over_plain_http_or_ssh() {
    let state = AppState::new().with_secrets(Box::new(MemoryStore::default()));
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

/// A store that says it survives a restart, as the OS keyring does.
#[derive(Default)]
struct Keyring(MemoryStore);

impl SecretStore for Keyring {
    fn get(&self, key: &str) -> Option<String> {
        self.0.get(key)
    }
    fn set(&self, key: &str, secret: &str) -> Result<(), app_state::SecretError> {
        self.0.set(key, secret)
    }
    fn delete(&self, key: &str) -> Result<(), app_state::SecretError> {
        self.0.delete(key)
    }
    fn persistent(&self) -> bool {
        true
    }
}

fn sign_in(host: &str, username: Option<&str>, secret: &str) -> SignIn {
    SignIn {
        host: host.to_owned(),
        username: username.map(str::to_owned),
        secret: secret.to_owned(),
        remember: true,
    }
}

#[test]
fn a_token_stored_before_sign_ins_reads_as_one_without_a_username() {
    let store = Keyring::default();
    store.set("gitlab.example", "glpat-old").unwrap();
    let state = AppState::new().with_secrets(Box::new(store));

    let credential = state
        .stored_credential("https://gitlab.example/o/r.git")
        .unwrap();

    assert_eq!(credential.username, None);
    assert_eq!(credential.secret, "glpat-old");
    let known = state.list_credentials().known;
    assert_eq!(known.len(), 1);
    assert_eq!(known[0].host, "gitlab.example");
}

#[test]
fn a_sign_in_keeps_its_username_beside_the_secret() {
    let state = AppState::new().with_secrets(Box::new(Keyring::default()));

    state
        .store_credential(&sign_in("https://Cdb.Example/o/r.git", Some("ann"), "pw"))
        .unwrap();

    let credential = state
        .stored_credential("https://cdb.example/x.git")
        .unwrap();
    assert_eq!(credential.username.as_deref(), Some("ann"));
    assert_eq!(credential.secret, "pw");
}

#[test]
fn the_list_names_hosts_and_users_and_never_a_secret() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("credentials.json");
    let state = AppState::new().with_secrets(Box::new(Keyring::default()));
    state.use_credential_list(file.clone());

    state
        .store_credential(&sign_in("b.example", Some("bob"), "secret-b"))
        .unwrap();
    state
        .store_credential(&sign_in("a.example", None, "secret-a"))
        .unwrap();

    let list = state.list_credentials();
    assert!(list.persistent);
    let shown: Vec<_> = list
        .known
        .iter()
        .map(|k| (k.host.as_str(), k.username.as_deref()))
        .collect();
    assert_eq!(shown, [("a.example", None), ("b.example", Some("bob"))]);
    let written = std::fs::read_to_string(&file).unwrap();
    assert!(
        written.contains("b.example") && !written.contains("secret"),
        "{written}"
    );

    let reopened = AppState::new().with_secrets(Box::new(Keyring::default()));
    reopened.use_credential_list(file);
    assert_eq!(reopened.list_credentials().known.len(), 2);
}

#[test]
fn forgetting_all_empties_the_keyring_and_the_list() {
    let state = AppState::new().with_secrets(Box::new(Keyring::default()));
    state
        .store_credential(&sign_in("a.example", None, "x"))
        .unwrap();
    state
        .store_credential(&sign_in("b.example", None, "y"))
        .unwrap();

    state.forget_credential("a.example").unwrap();
    assert!(state.stored_credential("https://a.example/r").is_none());
    assert_eq!(state.list_credentials().known.len(), 1);

    state.forget_all_credentials().unwrap();
    assert!(state.stored_credential("https://b.example/r").is_none());
    assert!(state.list_credentials().known.is_empty());
}

// Without Secret Service a sign-in lasts until Cogit exits; a list on disk would name it
// after it is gone.
#[test]
fn without_an_os_store_the_list_is_not_written() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("credentials.json");
    let state = AppState::new().with_secrets(Box::new(MemoryStore::default()));
    state.use_credential_list(file.clone());

    state
        .store_credential(&sign_in("a.example", None, "x"))
        .unwrap();

    assert!(!state.list_credentials().persistent);
    assert_eq!(state.list_credentials().known.len(), 1);
    assert!(!file.exists());
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
