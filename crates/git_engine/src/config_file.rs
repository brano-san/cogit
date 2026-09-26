//! Repository ▸ Edit Git Config (doc/12-risks.md, R-155).

use crate::runner::bare_git;
use crate::{GitError, RepoHandle, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Why git refused the text. `line` is 1-based, as git counts it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, specta::Type)]
#[error("{message}")]
#[serde(rename_all = "camelCase")]
pub struct ConfigProblem {
    pub line: Option<u32>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ConfigScope {
    Repository,
    User,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFile {
    pub path: String,
    /// Always `\n`; `crlf` says what to write back.
    pub text: String,
    pub crlf: bool,
    pub exists: bool,
}

impl RepoHandle {
    /// `--git-path`: `<root>/.git/config` is wrong for submodules and worktrees.
    pub fn config_path(&self) -> Result<PathBuf> {
        let output = self.run_git_reading(&["rev-parse", "--git-path", "config"])?;
        Ok(self.root().join(output.stdout.trim()))
    }
}

/// Asked of git; a config with no entries lists nothing, and then git's rules apply.
pub fn user_config_path() -> PathBuf {
    let listing = bare_git(&["config", "--global", "--show-origin", "--list"])
        .map(|output| output.stdout)
        .unwrap_or_default();
    origin_file(&listing).unwrap_or_else(|| {
        user_config_by_rules(&|name| std::env::var(name).ok(), &|path| path.is_file())
    })
}

#[must_use]
pub fn origin_file(listing: &str) -> Option<PathBuf> {
    listing
        .lines()
        .find_map(|line| line.strip_prefix("file:"))
        .and_then(|rest| rest.split('\t').next())
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

pub fn user_config_by_rules(
    env: &dyn Fn(&str) -> Option<String>,
    exists: &dyn Fn(&Path) -> bool,
) -> PathBuf {
    if let Some(explicit) = env("GIT_CONFIG_GLOBAL").filter(|value| !value.is_empty()) {
        return PathBuf::from(explicit);
    }
    let home = env("HOME")
        .or_else(|| env("USERPROFILE"))
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    let classic = home.join(".gitconfig");
    if exists(&classic) {
        return classic;
    }
    let xdg = env("XDG_CONFIG_HOME")
        .map_or_else(|| home.join(".config"), PathBuf::from)
        .join("git")
        .join("config");
    if exists(&xdg) { xdg } else { classic }
}

pub fn read_config(path: &Path) -> Result<ConfigFile> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => Some(raw),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => return Err(err.into()),
    };
    let crlf = raw.as_deref().is_some_and(|text| text.contains("\r\n"));
    Ok(ConfigFile {
        path: path.to_string_lossy().replace('\\', "/"),
        text: raw.as_deref().unwrap_or_default().replace("\r\n", "\n"),
        crlf,
        exists: raw.is_some(),
    })
}

/// Written the way git writes it (R-483): into `config.lock`, taken exclusively, then
/// renamed over the file. A `git config` in the middle of its own write keeps its lock and
/// the save fails, instead of one of the two changes being lost. Checked in the lock, beside
/// the file, so a relative `[include]` resolves as it will for real.
pub fn save_config(path: &Path, text: &str, crlf: bool) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = if crlf {
        text.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        text.to_owned()
    };
    let mut candidate = path.as_os_str().to_owned();
    candidate.push(".lock");
    let candidate = PathBuf::from(candidate);
    let mut lock = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&candidate)
        .map_err(|err| {
            GitError::Io(format!(
                "could not lock config file {}: {err}",
                path.display()
            ))
        })?;
    let written = std::io::Write::write_all(&mut lock, body.as_bytes());
    drop(lock);
    if let Err(err) = written {
        let _ = std::fs::remove_file(&candidate);
        return Err(err.into());
    }

    let checked = bare_git(&["config", "--file", &candidate.to_string_lossy(), "--list"]);
    let refusal = match checked {
        Ok(output) if output.exit_code == Some(0) => None,
        Ok(output) => Some(output.stderr),
        Err(err) => Some(err.to_string()),
    };
    if let Some(stderr) = refusal {
        let _ = std::fs::remove_file(&candidate);
        return Err(GitError::ConfigInvalid(ConfigProblem {
            line: bad_line(&stderr),
            message: stderr
                .trim()
                .replace(&*candidate.to_string_lossy(), &path.to_string_lossy()),
        }));
    }
    rename_when_let_go(&candidate, path).map_err(|err| {
        let _ = std::fs::remove_file(&candidate);
        GitError::from(err)
    })
}

/// A `git.exe` reading the file holds it without `FILE_SHARE_DELETE`, and Windows refuses
/// to replace it until it lets go; git's own `mingw_rename` waits that out too.
fn rename_when_let_go(from: &Path, to: &Path) -> std::io::Result<()> {
    const WAITS_MS: [u64; 7] = [10, 20, 50, 100, 200, 400, 800];
    let mut waits = WAITS_MS.iter();
    loop {
        let err = match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(err) => err,
        };
        // 32: ERROR_SHARING_VIOLATION, which has no `ErrorKind` of its own.
        let busy = err.kind() == std::io::ErrorKind::PermissionDenied
            || (cfg!(windows) && err.raw_os_error() == Some(32));
        match waits.next() {
            Some(ms) if busy => std::thread::sleep(std::time::Duration::from_millis(*ms)),
            _ => return Err(err),
        }
    }
}

/// `fatal: bad config line 3 in file …`
fn bad_line(stderr: &str) -> Option<u32> {
    let rest = &stderr[stderr.find("line ")? + "line ".len()..];
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}
