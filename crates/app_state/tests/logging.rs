// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::logging::{
    LOG_FILES_KEPT, LOG_SIZE_LIMIT, SessionLog, log_filter, read_log_level, start_label,
};

#[test]
fn the_default_filter_is_quiet_about_dependencies_and_loud_about_cogit() {
    let filter = log_filter(None);

    assert!(filter.contains("gix=warn"), "{filter}");
    assert!(filter.contains("notify=warn"), "{filter}");
    assert!(filter.contains("git_engine="), "{filter}");
}

#[test]
fn a_chosen_level_applies_to_every_cogit_crate() {
    let filter = log_filter(Some("trace"));

    assert!(filter.contains("git_engine=trace"), "{filter}");
    assert!(filter.contains("app_state=trace"), "{filter}");
}

#[test]
fn a_chosen_level_does_not_turn_the_dependencies_loud() {
    let filter = log_filter(Some("trace"));
    assert!(filter.contains("gix=warn"), "{filter}");
}

// Preferences showed `info` while a settings file without the key logged at `debug`, and
// saving any other preference then wrote `info` and changed the log behind the user's back.
#[test]
fn no_level_chosen_is_the_level_preferences_shows() {
    let frontend = include_str!("../../../frontend/src/lib/settings.ts");
    let defaults = &frontend[frontend.find("DEFAULT_SETTINGS: Settings").unwrap()..];
    let shown = defaults
        .split_once("logLevel: \"")
        .and_then(|(_, rest)| rest.split_once('"'))
        .map(|(level, _)| level)
        .unwrap();

    assert_eq!(log_filter(None), log_filter(Some(shown)));
}

#[test]
fn a_level_that_is_not_a_level_falls_back_to_the_default() {
    assert_eq!(log_filter(Some("shout")), log_filter(None));
}

#[test]
fn a_missing_settings_file_means_no_chosen_level() {
    let dir = tempfile::tempdir().unwrap();
    assert!(read_log_level(dir.path()).is_none());
}

#[test]
fn the_level_is_read_out_of_the_settings_the_ui_writes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("settings.json"),
        r#"{"settings":{"logLevel":"trace","theme":"dark"}}"#,
    )
    .unwrap();

    assert_eq!(read_log_level(dir.path()).as_deref(), Some("trace"));
}

#[test]
fn settings_that_are_not_json_cost_the_level_but_not_the_startup() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("settings.json"), "{ not json").unwrap();

    assert!(read_log_level(dir.path()).is_none());
}

#[test]
fn settings_without_a_level_mean_no_chosen_level() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("settings.json"), r#"{"settings":{}}"#).unwrap();

    assert!(read_log_level(dir.path()).is_none());
}

/// One file per run, named after the moment it started; a run past 10 MB goes on in a
/// second file of the same run; the folder never keeps more than ten (doc/12-risks.md, R-159).
fn names(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn write_lines(log: &mut SessionLog, total: usize) {
    use std::io::Write as _;
    let line = [b'x'; 100];
    for _ in 0..total / line.len() {
        log.write_all(&line).unwrap();
    }
    log.flush().unwrap();
}

#[test]
fn the_limits_are_ten_megabytes_and_ten_files() {
    assert_eq!(LOG_SIZE_LIMIT, 10 * 1024 * 1024);
    assert_eq!(LOG_FILES_KEPT, 10);
}

#[test]
fn the_log_is_named_after_the_moment_cogit_started() {
    let dir = tempfile::tempdir().unwrap();
    let log = SessionLog::start(dir.path(), "2026-09-22_23-15-04").unwrap();
    assert_eq!(names(dir.path()), vec!["cogit-2026-09-22_23-15-04.log"]);
    assert_eq!(log.path(), dir.path().join("cogit-2026-09-22_23-15-04.log"));
}

#[test]
fn the_start_label_is_a_local_time_a_file_name_can_hold() {
    let label = start_label();
    assert_eq!(label.len(), "2026-09-22_23-15-04".len(), "{label}");
    assert!(
        label
            .chars()
            .all(|c| c.is_ascii_digit() || c == '-' || c == '_'),
        "{label}"
    );
}

#[test]
fn a_run_past_the_limit_goes_on_in_the_next_part_of_the_same_run() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = SessionLog::with_limits(dir.path(), "2026-09-22_23-15-04", 1000, 10).unwrap();
    write_lines(&mut log, 2500);

    assert_eq!(
        names(dir.path()),
        vec![
            "cogit-2026-09-22_23-15-04.2.log",
            "cogit-2026-09-22_23-15-04.3.log",
            "cogit-2026-09-22_23-15-04.log",
        ]
    );
    for name in names(dir.path()) {
        let size = std::fs::metadata(dir.path().join(&name)).unwrap().len();
        assert!(size <= 1000, "{name} is {size} bytes");
    }
    assert_eq!(
        log.path(),
        dir.path().join("cogit-2026-09-22_23-15-04.3.log")
    );
}

#[test]
fn the_folder_never_holds_more_than_ten_logs_and_the_oldest_go_first() {
    let dir = tempfile::tempdir().unwrap();
    for day in 1..=12 {
        let name = format!("cogit-2026-09-{day:02}_10-00-00.log");
        std::fs::write(dir.path().join(name), b"old").unwrap();
    }

    let _log = SessionLog::start(dir.path(), "2026-09-22_23-15-04").unwrap();

    let left = names(dir.path());
    assert_eq!(left.len(), 10, "{left:?}");
    assert!(left.contains(&"cogit-2026-09-22_23-15-04.log".to_owned()));
    assert!(!left.contains(&"cogit-2026-09-01_10-00-00.log".to_owned()));
    assert!(!left.contains(&"cogit-2026-09-03_10-00-00.log".to_owned()));
    assert!(left.contains(&"cogit-2026-09-04_10-00-00.log".to_owned()));
}

#[test]
fn a_long_run_keeps_its_newest_parts_within_the_cap() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = SessionLog::with_limits(dir.path(), "2026-09-22_23-15-04", 1000, 3).unwrap();
    write_lines(&mut log, 10_000);

    let left = names(dir.path());
    assert_eq!(left.len(), 3, "{left:?}");
    assert!(
        log.path().exists(),
        "the file being written is never the one removed"
    );
}

#[test]
fn files_that_are_not_rotating_logs_are_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("panic.log"), b"boom").unwrap();
    std::fs::write(dir.path().join("settings.json"), b"{}").unwrap();

    let mut log = SessionLog::with_limits(dir.path(), "2026-09-22_23-15-04", 1000, 1).unwrap();
    write_lines(&mut log, 5000);

    let left = names(dir.path());
    assert!(left.contains(&"panic.log".to_owned()));
    assert!(left.contains(&"settings.json".to_owned()));
}

#[test]
fn logs_from_before_the_named_files_are_the_first_to_go() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["cogit.log", "cogit.log.1", "cogit.log.2"] {
        std::fs::write(dir.path().join(name), b"legacy").unwrap();
    }
    // Three old-style files, nine named ones and the new run: three have to go.
    for day in 1..=9 {
        let name = format!("cogit-2026-09-{day:02}_10-00-00.log");
        std::fs::write(dir.path().join(name), b"old").unwrap();
    }

    let _log = SessionLog::start(dir.path(), "2026-09-22_23-15-04").unwrap();

    let left = names(dir.path());
    assert_eq!(left.len(), 10);
    assert!(
        !left.iter().any(|name| name.starts_with("cogit.log")),
        "{left:?}"
    );
    assert!(
        left.contains(&"cogit-2026-09-01_10-00-00.log".to_owned()),
        "{left:?}"
    );
}

#[test]
fn a_second_start_in_the_same_second_does_not_share_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let first = SessionLog::start(dir.path(), "2026-09-22_23-15-04").unwrap();
    let second = SessionLog::start(dir.path(), "2026-09-22_23-15-04").unwrap();
    assert_ne!(first.path(), second.path());
}

#[test]
fn the_profile_target_survives_a_quiet_log_level() {
    for level in ["error", "warn", "info", "debug", "trace"] {
        let filter = log_filter(Some(level));
        assert!(
            filter.contains("cogit::profile=info"),
            "a day of profiling must survive {level}: {filter}"
        );
    }
}

// A crate the filter does not name logs nothing at all: every `warn!` of the avatar cache
// (an index that cannot be written, a picture that cannot be saved) was dropped.
#[test]
fn the_avatar_cache_is_in_the_log() {
    assert!(
        log_filter(None).contains("avatars=info"),
        "{}",
        log_filter(None)
    );
    assert!(log_filter(Some("debug")).contains("avatars=debug"));
}

// The path kept at start-up is part one. Past 10 MB the session writes part two, and
// Copy Diagnostics went on sending the tail of part one, without the failure in it.
#[test]
fn the_latest_part_of_a_session_is_the_one_being_written() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("cogit-2026-01-01_00-00-00.log");
    std::fs::write(&first, "old\n").unwrap();
    std::fs::write(
        dir.path().join("cogit-2026-01-01_00-00-00.2.log"),
        "newer\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("cogit-2026-01-01_00-00-00.10.log"),
        "newest\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("cogit-2027-01-01_00-00-00.3.log"),
        "another run\n",
    )
    .unwrap();

    assert_eq!(
        app_state::logging::latest_part(&first),
        dir.path().join("cogit-2026-01-01_00-00-00.10.log")
    );
}

#[test]
fn a_session_that_never_rolled_is_its_first_part() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("cogit-2026-01-01_00-00-00.log");
    std::fs::write(&first, "only\n").unwrap();

    assert_eq!(app_state::logging::latest_part(&first), first);
}
