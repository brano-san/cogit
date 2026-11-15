//! `cogit [<path>…]`, `cogit log [<path>]`, `cogit blame <file>`: for this process and for
//! every later one handed over to it.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Show {
    Repository,
    /// The history of a file (Investigate), or the repository's graph for a folder.
    Log,
    Blame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub show: Show,
    pub paths: Vec<OsString>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub show: Show,
    pub paths: Vec<PathBuf>,
}

/// Options before `--` are skipped: Cogit's were handled earlier, the system adds its own.
pub fn parse<I: IntoIterator<Item = OsString>>(args: I) -> Result<Option<Command>, String> {
    let mut words = Vec::new();
    let mut options_end = false;
    let mut may_be_command = true;
    for arg in args {
        if !options_end && arg == "--" {
            options_end = true;
            may_be_command = !words.is_empty();
        } else if options_end || !arg.to_string_lossy().starts_with('-') {
            words.push(arg);
        }
    }
    let first = words.first().filter(|_| may_be_command);
    let (show, paths) = match first.and_then(|word| word.to_str()) {
        Some("log") => (Show::Log, words.split_off(1)),
        Some("blame") => (Show::Blame, words.split_off(1)),
        _ => (Show::Repository, words),
    };
    match (show, paths.len()) {
        (Show::Repository, 0) => Ok(None),
        (Show::Blame, 0) => Err("cogit blame: name the file to blame".to_owned()),
        (Show::Log | Show::Blame, 2..) => Err(format!(
            "cogit {}: one path at most",
            if show == Show::Log { "log" } else { "blame" }
        )),
        _ => Ok(Some(Command { show, paths })),
    }
}

pub fn resolve(command: Command, cwd: &Path) -> Request {
    let mut paths: Vec<PathBuf> = command
        .paths
        .iter()
        .map(|path| absolute(cwd, &unquote(path, cfg!(windows))))
        .collect();
    if paths.is_empty() {
        paths.push(normalize(cwd));
    }
    Request {
        show: command.show,
        paths,
    }
}

/// Explorer's `"%V"` on a drive root is `"C:\"`, and Windows reads `\"` as an escaped quote:
/// the argument arrives as `C:"`. No Windows path holds a `"`: it was that backslash.
pub fn unquote(arg: &OsString, windows: bool) -> OsString {
    match arg.to_str() {
        Some(text) if windows && text.ends_with('"') => {
            format!("{}\\", text.trim_end_matches('"')).into()
        }
        _ => arg.clone(),
    }
}

pub fn absolute(cwd: &Path, path: &OsString) -> PathBuf {
    normalize(&cwd.join(path))
}

pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(out.components().next_back(), Some(Component::Normal(_))) {
                    out.pop();
                }
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;
