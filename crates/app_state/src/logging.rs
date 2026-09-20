use file_rotate::{ContentLimit, FileRotate, compression::Compression, suffix::AppendCount};
use std::path::Path;

pub const LOG_SIZE_LIMIT: usize = 10 * 1024 * 1024;
pub const LOG_ARCHIVES: usize = 2;

const COGIT_CRATES: &[&str] = &[
    "cogit",
    "cogit_lib",
    "git_engine",
    "diff_engine",
    "graph_engine",
    "fs_watcher",
    "app_state",
];

const QUIET: &str = "gix=warn,notify=warn,tauri=info";

/// The profiling stream ignores the chosen level: its whole point is that a day of ordinary
/// use leaves a log worth reading back, whatever the user set.
const PROFILE: &str = "cogit::profile=info";
const LEVELS: &[&str] = &["error", "warn", "info", "debug", "trace"];

#[must_use]
pub fn rotating_writer(log_dir: &Path) -> FileRotate<AppendCount> {
    FileRotate::new(
        log_dir.join("cogit.log"),
        AppendCount::new(LOG_ARCHIVES),
        ContentLimit::Bytes(LOG_SIZE_LIMIT),
        Compression::None,
        None,
    )
}

/// Applies to Cogit's crates only: `gix` at trace would bury the log.
#[must_use]
pub fn log_filter(level: Option<&str>) -> String {
    let chosen = level
        .filter(|value| LEVELS.contains(value))
        .unwrap_or("debug");
    let mut parts: Vec<String> = COGIT_CRATES
        .iter()
        .map(|krate| format!("{krate}={chosen}"))
        .collect();
    parts.push(QUIET.to_owned());
    parts.push(PROFILE.to_owned());
    parts.join(",")
}

/// Read straight from the file the settings panel writes, before the webview exists.
#[must_use]
pub fn read_log_level(config_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(config_dir.join("settings.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let level = value.get("settings")?.get("logLevel")?.as_str()?;
    LEVELS.contains(&level).then(|| level.to_owned())
}
