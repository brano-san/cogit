//! Debounced, filtered filesystem watching for Git repositories.
//!
//! Scope limits are INV-06: watching a repository root recursively exhausts OS handles
//! on large monorepos, and `.git/objects` alone produces thousands of events during a
//! single `fetch`.

mod watcher;

pub use watcher::{DEFAULT_QUIET, RepoWatcher};

use serde::{Deserialize, Serialize};
use std::path::Path;

pub const DEBOUNCE_MS: u64 = 100;

pub const WATCHED_GIT_PATHS: &[&str] = &[
    "HEAD",
    "index",
    "refs",
    "MERGE_HEAD",
    "CHERRY_PICK_HEAD",
    "REVERT_HEAD",
    "rebase-merge",
    "rebase-apply",
];

pub const EXCLUDED_DIRS: &[&str] = &[
    "objects",
    "target",
    "build",
    "node_modules",
    "dist",
    ".venv",
    "__pycache__",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    Head,
    Index,
    Refs,
    WorkingTree,
    Stash,
    Config,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RepoChanged {
    pub kind: ChangeKind,
    pub path: String,
}

#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    #[error("failed to start watching {path}: {source}")]
    Start { path: String, source: notify::Error },
}

#[must_use]
pub fn is_excluded(path: &Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|name| EXCLUDED_DIRS.contains(&name))
    })
}

#[must_use]
pub fn classify_git_path(relative: &str) -> Option<ChangeKind> {
    let normalized = relative.replace('\\', "/");
    let normalized = normalized.trim_start_matches('/');

    if normalized == "HEAD" || normalized == "ORIG_HEAD" {
        return Some(ChangeKind::Head);
    }
    if normalized == "index" {
        return Some(ChangeKind::Index);
    }
    if normalized == "config" {
        return Some(ChangeKind::Config);
    }
    if normalized == "refs/stash" || normalized.starts_with("logs/refs/stash") {
        return Some(ChangeKind::Stash);
    }
    if normalized.starts_with("refs/") || normalized == "packed-refs" {
        return Some(ChangeKind::Refs);
    }
    if normalized.starts_with("MERGE_HEAD")
        || normalized.starts_with("CHERRY_PICK_HEAD")
        || normalized.starts_with("REVERT_HEAD")
        || normalized.starts_with("rebase-merge")
        || normalized.starts_with("rebase-apply")
    {
        return Some(ChangeKind::Head);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn git_objects_are_never_watched() {
        // A single fetch touches thousands of files under objects/ (INV-06).
        assert!(is_excluded(&PathBuf::from(".git/objects/ab/cdef")));
    }

    #[test]
    fn build_output_directories_are_excluded() {
        assert!(is_excluded(&PathBuf::from("target/debug/build")));
        assert!(is_excluded(&PathBuf::from("frontend/node_modules/svelte")));
        assert!(!is_excluded(&PathBuf::from("src/main.rs")));
    }

    #[test]
    fn head_and_index_map_to_their_own_kinds() {
        assert_eq!(classify_git_path("HEAD"), Some(ChangeKind::Head));
        assert_eq!(classify_git_path("index"), Some(ChangeKind::Index));
    }

    #[test]
    fn stash_is_distinguished_from_ordinary_refs() {
        assert_eq!(classify_git_path("refs/stash"), Some(ChangeKind::Stash));
        assert_eq!(classify_git_path("refs/heads/main"), Some(ChangeKind::Refs));
    }

    #[test]
    fn in_progress_operations_report_as_head_changes() {
        assert_eq!(classify_git_path("MERGE_HEAD"), Some(ChangeKind::Head));
        assert_eq!(
            classify_git_path("rebase-merge/done"),
            Some(ChangeKind::Head)
        );
    }

    #[test]
    fn windows_separators_are_handled() {
        assert_eq!(
            classify_git_path(r"refs\heads\main"),
            Some(ChangeKind::Refs)
        );
    }

    #[test]
    fn unknown_paths_produce_no_event() {
        assert_eq!(classify_git_path("COMMIT_EDITMSG"), None);
    }
}
