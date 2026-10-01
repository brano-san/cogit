use std::path::{Path, PathBuf};

pub const LEGACY_IDENTIFIER: &str = "dev.branosan.cogit";
pub const IDENTIFIER: &str = "Cogit";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Migration {
    Absent,
    Renamed,
    /// The new folder already existed (the NSIS install folder is `%LOCALAPPDATA%\Cogit`):
    /// entries it lacks were moved, the rest stay in the old folder.
    Merged {
        moved: Vec<String>,
        left: Vec<String>,
    },
    Failed(String),
}

/// Moves `old` to `new`, never deleting or overwriting anything.
pub fn migrate_legacy_dirs(pairs: &[(PathBuf, PathBuf)]) -> Vec<(PathBuf, Migration)> {
    pairs
        .iter()
        .map(|(old, new)| (old.clone(), migrate(old, new)))
        .collect()
}

fn migrate(old: &Path, new: &Path) -> Migration {
    if !old.is_dir() {
        return Migration::Absent;
    }
    if !new.exists() {
        return match std::fs::rename(old, new) {
            Ok(()) => Migration::Renamed,
            Err(err) => Migration::Failed(err.to_string()),
        };
    }
    let entries = match std::fs::read_dir(old) {
        Ok(entries) => entries,
        Err(err) => return Migration::Failed(err.to_string()),
    };
    let (mut moved, mut left) = (Vec::new(), Vec::new());
    for entry in entries.flatten() {
        let name = entry.file_name();
        let label = name.to_string_lossy().into_owned();
        let target = new.join(&name);
        if !target.exists() && std::fs::rename(entry.path(), &target).is_ok() {
            moved.push(label);
        } else {
            left.push(label);
        }
    }
    if left.is_empty() {
        let _ = std::fs::remove_dir(old);
    }
    Migration::Merged { moved, left }
}

/// The folders Tauri derives from the identifier, per platform; duplicates are dropped.
#[must_use]
pub fn app_folder_pairs(old: &str, new: &str) -> Vec<(PathBuf, PathBuf)> {
    let mut pairs: Vec<(PathBuf, PathBuf)> = Vec::new();
    for base in base_dirs() {
        let pair = (base.join(old), base.join(new));
        if !pairs.contains(&pair) {
            pairs.push(pair);
        }
    }
    pairs
}

fn base_dirs() -> Vec<PathBuf> {
    let env = |name: &str| {
        std::env::var_os(name)
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
    };
    let home = || env("HOME");
    if cfg!(windows) {
        ["APPDATA", "LOCALAPPDATA"]
            .iter()
            .filter_map(|n| env(n))
            .collect()
    } else if cfg!(target_os = "macos") {
        home()
            .map(|h| {
                vec![
                    h.join("Library/Application Support"),
                    h.join("Library/Caches"),
                    h.join("Library/Logs"),
                ]
            })
            .unwrap_or_default()
    } else {
        [
            ("XDG_CONFIG_HOME", ".config"),
            ("XDG_DATA_HOME", ".local/share"),
            ("XDG_CACHE_HOME", ".cache"),
        ]
        .iter()
        .filter_map(|(var, fallback)| env(var).or_else(|| home().map(|h| h.join(fallback))))
        .collect()
    }
}

pub fn log_outcomes(outcomes: &[(PathBuf, Migration)]) {
    for (old, outcome) in outcomes {
        match outcome {
            Migration::Absent => {}
            Migration::Renamed => {
                tracing::info!(folder = %old.display(), "moved a legacy app folder to its new name")
            }
            Migration::Merged { moved, left } if left.is_empty() => {
                tracing::info!(folder = %old.display(), ?moved, "moved legacy app folder entries into the existing folder");
            }
            Migration::Merged { moved, left } => {
                tracing::warn!(folder = %old.display(), ?moved, ?left, "legacy app folder entries clash with the new folder and stay in place");
            }
            Migration::Failed(error) => {
                tracing::warn!(folder = %old.display(), error, "cannot move a legacy app folder; it is left as it is");
            }
        }
    }
}
