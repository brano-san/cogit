// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{MemoryStore, SecretStore, host_of};

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
