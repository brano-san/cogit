// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(unsafe_code, clippy::unwrap_used, clippy::expect_used)]

//! Preferences ▸ Network (F-942): proxy and TLS keys in the user's global Git config.

use git_engine::{CommandSink, GitOutput, global_network_settings, set_global_setting};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// A global config of the test's own; nextest gives each test its own process.
fn own_global(dir: &Path) -> PathBuf {
    let global = dir.join("gitconfig");
    // SAFETY: nextest runs each test in its own process, and this is its only thread then.
    unsafe {
        std::env::set_var("GIT_CONFIG_GLOBAL", &global);
    }
    global
}

fn value(key: &str) -> Option<String> {
    global_network_settings()
        .unwrap()
        .into_iter()
        .find(|entry| entry.key == key)
        .unwrap()
        .value
}

fn journal() -> (CommandSink, Arc<Mutex<Vec<GitOutput>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    let journal: CommandSink = Arc::new(move |entry| sink.lock().unwrap().push(entry));
    (journal, seen)
}

#[test]
fn nothing_set_reads_as_three_unset_keys_even_without_a_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let global = own_global(dir.path());
    assert!(!global.exists());

    let keys: Vec<(String, Option<String>)> = global_network_settings()
        .unwrap()
        .into_iter()
        .map(|entry| (entry.key, entry.value))
        .collect();

    assert_eq!(
        keys,
        [
            ("http.proxy".to_owned(), None),
            ("http.sslVerify".to_owned(), None),
            ("http.sslCAInfo".to_owned(), None),
        ]
    );
}

#[test]
fn a_value_is_written_to_the_global_file_and_empty_removes_it() {
    let dir = tempfile::TempDir::new().unwrap();
    let global = own_global(dir.path());

    set_global_setting("http.sslverify", Some("false"), None).unwrap();
    set_global_setting("http.sslCAInfo", Some(" C:/certs/corp.pem "), None).unwrap();

    assert_eq!(value("http.sslVerify").as_deref(), Some("false"));
    assert_eq!(
        value("http.sslCAInfo").as_deref(),
        Some("C:/certs/corp.pem")
    );
    assert!(
        std::fs::read_to_string(&global)
            .unwrap()
            .contains("sslVerify")
    );

    set_global_setting("http.sslVerify", None, None).unwrap();
    set_global_setting("http.sslCAInfo", Some("  "), None).unwrap();

    assert_eq!(value("http.sslVerify"), None);
    assert_eq!(value("http.sslCAInfo"), None);
}

#[test]
fn removing_what_is_not_there_runs_nothing() {
    let dir = tempfile::TempDir::new().unwrap();
    own_global(dir.path());
    let (sink, seen) = journal();

    set_global_setting("http.proxy", None, Some(&sink)).unwrap();

    assert!(seen.lock().unwrap().is_empty());
}

#[test]
fn only_the_offered_keys_are_written() {
    let dir = tempfile::TempDir::new().unwrap();
    own_global(dir.path());

    let err = set_global_setting("core.editor", Some("vim"), None).unwrap_err();

    assert!(err.to_string().contains("core.editor"), "{err}");
}

#[test]
fn the_journal_shows_the_proxy_with_its_password_hidden() {
    let dir = tempfile::TempDir::new().unwrap();
    own_global(dir.path());
    let (sink, seen) = journal();

    set_global_setting(
        "http.proxy",
        Some("bob:hunter2@proxy.corp:3128"),
        Some(&sink),
    )
    .unwrap();

    assert_eq!(
        value("http.proxy").as_deref(),
        Some("bob:hunter2@proxy.corp:3128")
    );
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert!(!seen[0].command.contains("hunter2"), "{}", seen[0].command);
    assert!(
        seen[0].command.contains("proxy.corp:3128"),
        "{}",
        seen[0].command
    );
}
