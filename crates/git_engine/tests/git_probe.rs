use git_engine::{parse_git_version, probe_git};
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(10);

#[test]
fn version_keeps_the_numbers_and_drops_the_vendor_suffix() {
    assert_eq!(
        parse_git_version("git version 2.41.0"),
        Some("2.41.0".into())
    );
    assert_eq!(
        parse_git_version("git version 2.41.0.windows.1\n"),
        Some("2.41.0".into())
    );
    assert_eq!(
        parse_git_version("git version 2.39.3 (Apple Git-146)"),
        Some("2.39.3".into())
    );
    assert_eq!(parse_git_version("hello"), None);
    assert_eq!(parse_git_version(""), None);
}

#[test]
fn the_git_on_path_is_valid() {
    let probe = probe_git("git", WAIT);
    assert!(probe.valid, "{probe:?}");
    assert!(
        probe
            .version
            .is_some_and(|v| v.starts_with(|c: char| c.is_ascii_digit()))
    );
    assert_eq!(probe.error, None);
}

#[test]
fn an_empty_path_means_the_git_on_path() {
    assert!(probe_git("  ", WAIT).valid);
}

#[test]
fn a_missing_binary_is_invalid_and_says_why() {
    let probe = probe_git("definitely/not/a/git-binary", WAIT);
    assert!(!probe.valid);
    assert_eq!(probe.version, None);
    assert!(probe.error.is_some_and(|e| e.contains("definitely")));
}

#[cfg(unix)]
#[test]
fn a_program_that_hangs_is_stopped_at_the_timeout() {
    use std::os::unix::fs::PermissionsExt as _;
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("slow-git");
    std::fs::write(&script, "#!/bin/sh\nsleep 30\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let started = Instant::now();
    let probe = probe_git(script.to_str().unwrap(), Duration::from_millis(300));
    assert!(!probe.valid);
    assert!(probe.error.is_some_and(|e| e.contains("timed out")));
    assert!(started.elapsed() < Duration::from_secs(10));
}
