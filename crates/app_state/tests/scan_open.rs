#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::AppState;

#[test]
fn a_scan_marks_an_open_bare_repository_as_already_open() {
    let f = test_fixtures::linear(1).unwrap();
    let parent = tempfile::tempdir().unwrap();
    let bare = parent.path().join("lib.git");
    let cloned = test_fixtures::git_command_in(parent.path())
        .args(["clone", "-q", "--bare"])
        .arg(f.path())
        .arg(&bare)
        .status()
        .unwrap();
    assert!(cloned.success());
    let state = AppState::new();
    state.open_repository(&bare).unwrap();

    let mut found = Vec::new();
    state
        .scan_for_repositories(
            parent.path(),
            2,
            || false,
            |hit| {
                found.push(hit);
                true
            },
        )
        .unwrap();

    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].bare && found[0].already_open, "{found:?}");
}
