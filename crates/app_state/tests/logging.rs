// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::logging::{
    LOG_ARCHIVES, LOG_SIZE_LIMIT, log_filter, read_log_level, rotating_writer,
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

/// The DoD of T1.4: writing past the limit leaves the current file plus its archives.
#[test]
fn the_log_is_capped_and_keeps_at_most_two_archives() {
    use std::io::Write as _;

    let dir = tempfile::tempdir().unwrap();
    let mut writer = rotating_writer(dir.path());
    let line = vec![b'x'; 64 * 1024];

    let mut written = 0;
    while written < LOG_SIZE_LIMIT * 3 {
        writer.write_all(&line).unwrap();
        written += line.len();
    }
    writer.flush().unwrap();
    drop(writer);

    let files: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(std::result::Result::ok)
        .collect();
    let total: u64 = files
        .iter()
        .filter_map(|f| f.metadata().ok())
        .map(|m| m.len())
        .sum();

    assert!(
        files.len() <= LOG_ARCHIVES + 1,
        "{} files: {:?}",
        files.len(),
        files
            .iter()
            .map(std::fs::DirEntry::file_name)
            .collect::<Vec<_>>()
    );
    assert!(
        total <= (LOG_SIZE_LIMIT * (LOG_ARCHIVES + 1)) as u64,
        "{total} bytes after writing {written}"
    );
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
