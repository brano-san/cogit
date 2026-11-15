// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(unsafe_code, clippy::unwrap_used)]

//! Preferences ▸ Network (F-942): written to the global config, recorded in Output.

use app_state::AppState;

#[test]
fn a_global_proxy_is_written_and_journalled_without_its_password() {
    let dir = tempfile::TempDir::new().unwrap();
    // SAFETY: nextest runs each test in its own process, and this is its only thread then.
    unsafe {
        std::env::set_var("GIT_CONFIG_GLOBAL", dir.path().join("gitconfig"));
    }
    let state = AppState::new();

    state
        .set_global_network_setting("http.proxy", Some("http://bob:hunter2@proxy.corp:3128"))
        .unwrap();

    let read = state.global_network_settings().unwrap();
    assert_eq!(
        read[0].value.as_deref(),
        Some("http://bob:hunter2@proxy.corp:3128")
    );
    let log = state.command_log();
    let entry = log
        .iter()
        .find(|entry| entry.command.contains("http.proxy"))
        .unwrap();
    assert!(!entry.command.contains("hunter2"), "{}", entry.command);
}
