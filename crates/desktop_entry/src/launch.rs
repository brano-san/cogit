//! `cogit [<path>…]`: for this process and for every later one handed over to it.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Show {
    Repository,
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
    let mut paths = Vec::new();
    let mut options_end = false;
    for arg in args {
        if !options_end && arg == "--" {
            options_end = true;
        } else if options_end || !arg.to_string_lossy().starts_with('-') {
            paths.push(arg);
        }
    }
    if paths.is_empty() {
        return Ok(None);
    }
    Ok(Some(Command {
        show: Show::Repository,
        paths,
    }))
}

pub fn resolve(command: Command, cwd: &Path) -> Request {
    let paths: Vec<PathBuf> = command
        .paths
        .iter()
        .map(|path| absolute(cwd, &unquote(path, cfg!(windows))))
        .collect();
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
