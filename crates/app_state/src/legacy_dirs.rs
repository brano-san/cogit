use std::path::{Path, PathBuf};

pub const LEGACY_IDENTIFIER: &str = "dev.branosan.cogit";
pub const IDENTIFIER: &str = "Cogit";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Migration {
    Absent,
    Renamed,
    /// The new folder already existed (the NSIS install folder is `%LOCALAPPDATA%\Cogit`):
    /// entries it lacks were moved. `left` clashed with an entry of the new folder; `failed`
    /// could not be renamed (still open), and the new folder now holds an empty twin of each.
    Merged {
        moved: Vec<String>,
        left: Vec<String>,
        failed: Vec<String>,
    },
    Failed(String),
}

type Rename<'a> = &'a dyn Fn(&Path, &Path) -> std::io::Result<()>;

const TRIES: u32 = 5;
const PAUSE: std::time::Duration = std::time::Duration::from_millis(200);

/// Moves `old` to `new`, never deleting or overwriting anything.
pub fn migrate_legacy_dirs(pairs: &[(PathBuf, PathBuf)]) -> Vec<(PathBuf, Migration)> {
    migrate_with(pairs, &|from, to| std::fs::rename(from, to), PAUSE)
}

/// `migrate_legacy_dirs` with the rename and the pause between tries given, for tests.
pub fn migrate_with(
    pairs: &[(PathBuf, PathBuf)],
    rename: Rename<'_>,
    pause: std::time::Duration,
) -> Vec<(PathBuf, Migration)> {
    pairs
        .iter()
        .map(|(old, new)| (old.clone(), migrate(old, new, rename, pause)))
        .collect()
}

/// The old process's WebView2 outlives its installer by a few seconds and holds files open:
/// a rename that fails now is likely to work shortly. Before any window exists, so waiting is fine.
fn rename_retrying(
    rename: Rename<'_>,
    from: &Path,
    to: &Path,
    pause: std::time::Duration,
) -> std::io::Result<()> {
    let mut result = rename(from, to);
    for _ in 1..TRIES {
        if result.is_ok() {
            break;
        }
        std::thread::sleep(pause);
        result = rename(from, to);
    }
    result
}

fn migrate(old: &Path, new: &Path, rename: Rename<'_>, pause: std::time::Duration) -> Migration {
    if !old.is_dir() {
        return Migration::Absent;
    }
    if !new.exists() {
        match rename_retrying(rename, old, new, pause) {
            Ok(()) => return Migration::Renamed,
            // Whatever is not held open still moves, entry by entry. `create_dir`, not `_all`:
            // a missing parent is a real failure, not something to invent.
            Err(err) => {
                if std::fs::create_dir(new).is_err() {
                    return Migration::Failed(err.to_string());
                }
            }
        }
    }
    let entries = match std::fs::read_dir(old) {
        Ok(entries) => entries,
        Err(err) => return Migration::Failed(err.to_string()),
    };
    let (mut moved, mut left, mut failed) = (Vec::new(), Vec::new(), Vec::new());
    for entry in entries.flatten() {
        let name = entry.file_name();
        let label = name.to_string_lossy().into_owned();
        let target = new.join(&name);
        if target.exists() && !clear_empty_twin(&target) {
            left.push(label);
        } else if rename_retrying(rename, &entry.path(), &target, pause).is_ok() {
            moved.push(label);
        } else {
            failed.push(label);
        }
    }
    if left.is_empty() && failed.is_empty() {
        let _ = std::fs::remove_dir(old);
    }
    Migration::Merged {
        moved,
        left,
        failed,
    }
}

/// A run that could not move an entry still started the app, which made its own empty one
/// (a fresh settings file, an empty profile folder). Such a twin holds nothing to lose, so it
/// does not make the old entry a clash for ever. Removed only when it is empty.
fn clear_empty_twin(target: &Path) -> bool {
    let Ok(meta) = std::fs::symlink_metadata(target) else {
        return false;
    };
    if meta.is_dir() {
        std::fs::remove_dir(target).is_ok()
    } else {
        meta.is_file() && meta.len() == 0 && std::fs::remove_file(target).is_ok()
    }
}

/// What the user is told when an entry of an old folder is still there: nothing moved by
/// itself is worth a message, and an entry left alone is how a reset-looking start is explained.
#[must_use]
pub fn leftovers_note(outcomes: &[(PathBuf, Migration)]) -> Option<String> {
    let lines: Vec<String> = outcomes
        .iter()
        .filter_map(|(old, outcome)| match outcome {
            Migration::Merged { left, failed, .. } if !left.is_empty() || !failed.is_empty() => {
                let mut line = format!("{}:", old.display());
                if !failed.is_empty() {
                    line += &format!(" could not be moved (in use): {}.", failed.join(", "));
                }
                if !left.is_empty() {
                    line += &format!(
                        " also exist in the new folder, which wins: {}.",
                        left.join(", ")
                    );
                }
                Some(line)
            }
            Migration::Failed(error) => {
                Some(format!("{}: could not be moved: {error}", old.display()))
            }
            _ => None,
        })
        .collect();
    (!lines.is_empty()).then(|| {
        format!(
            "Settings of an earlier version were left in their old folder. Close other Cogit windows and start again to retry, or copy what you need.
{}",
            lines.join("
")
        )
    })
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
            Migration::Merged {
                moved,
                left,
                failed,
            } if left.is_empty() && failed.is_empty() => {
                tracing::info!(folder = %old.display(), ?moved, "moved legacy app folder entries into the existing folder");
            }
            Migration::Merged {
                moved,
                left,
                failed,
            } => {
                tracing::warn!(folder = %old.display(), ?moved, ?left, ?failed, "legacy app folder entries stay in place: `left` clash with the new folder, `failed` could not be moved");
            }
            Migration::Failed(error) => {
                tracing::warn!(folder = %old.display(), error, "cannot move a legacy app folder; it is left as it is");
            }
        }
    }
}
