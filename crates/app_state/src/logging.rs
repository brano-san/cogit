use std::io::Write;
use std::path::{Path, PathBuf};

pub const LOG_SIZE_LIMIT: usize = 10 * 1024 * 1024;
pub const LOG_FILES_KEPT: usize = 10;

const COGIT_CRATES: &[&str] = &[
    "cogit",
    "cogit_lib",
    "git_engine",
    "diff_engine",
    "graph_engine",
    "fs_watcher",
    "app_state",
    "avatars",
];

const QUIET: &str = "gix=warn,notify=warn,tauri=info";

/// The profiling stream ignores the chosen level: its whole point is that a day of ordinary
/// use leaves a log worth reading back, whatever the user set.
const PROFILE: &str = "cogit::profile=info";
const LEVELS: &[&str] = &["error", "warn", "info", "debug", "trace"];

#[must_use]
pub fn start_label() -> String {
    chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string()
}

/// `cogit-<start>.log`, then `cogit-<start>.2.log` past the limit (doc/12-risks.md, R-159).
#[derive(Debug)]
pub struct SessionLog {
    dir: PathBuf,
    started: String,
    part: u32,
    file: std::fs::File,
    written: usize,
    limit: usize,
    keep: usize,
}

impl SessionLog {
    pub fn start(dir: &Path, started: &str) -> std::io::Result<Self> {
        Self::with_limits(dir, started, LOG_SIZE_LIMIT, LOG_FILES_KEPT)
    }

    pub fn with_limits(
        dir: &Path,
        started: &str,
        limit: usize,
        keep: usize,
    ) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let mut part = 1;
        while dir.join(file_name(started, part)).exists() {
            part += 1;
        }
        let file = std::fs::File::create(dir.join(file_name(started, part)))?;
        let log = Self {
            dir: dir.to_path_buf(),
            started: started.to_owned(),
            part,
            file,
            written: 0,
            limit,
            keep: keep.max(1),
        };
        log.prune();
        Ok(log)
    }

    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.dir.join(file_name(&self.started, self.part))
    }

    fn roll(&mut self) -> std::io::Result<()> {
        self.file.flush()?;
        self.part += 1;
        self.file = std::fs::File::create(self.path())?;
        self.written = 0;
        self.prune();
        Ok(())
    }

    /// Oldest first by name; earlier versions' `cogit.log*` sort before everything.
    fn prune(&self) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        let current = self.path();
        let mut logs: Vec<(Option<(String, u32)>, PathBuf)> = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let order = if let Some(key) = parse_name(&name) {
                    Some(key)
                } else if name == "cogit.log" || name.starts_with("cogit.log.") {
                    None
                } else {
                    return None;
                };
                Some((order, entry.path()))
            })
            .collect();
        logs.sort();
        let excess = logs.len().saturating_sub(self.keep);
        for (_, path) in logs
            .into_iter()
            .filter(|(_, path)| *path != current)
            .take(excess)
        {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn file_name(started: &str, part: u32) -> String {
    if part <= 1 {
        format!("cogit-{started}.log")
    } else {
        format!("cogit-{started}.{part}.log")
    }
}

fn parse_name(name: &str) -> Option<(String, u32)> {
    let stem = name.strip_prefix("cogit-")?.strip_suffix(".log")?;
    match stem.split_once('.') {
        None => Some((stem.to_owned(), 1)),
        Some((started, part)) => Some((started.to_owned(), part.parse().ok()?)),
    }
}

impl Write for SessionLog {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.written > 0 && self.written + buf.len() > self.limit {
            self.roll()?;
        }
        let wrote = self.file.write(buf)?;
        self.written += wrote;
        Ok(wrote)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
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
    let level = crate::settings::read_document(config_dir)
        .get("settings")?
        .get("logLevel")?
        .as_str()?
        .to_owned();
    LEVELS.contains(&level.as_str()).then_some(level)
}
