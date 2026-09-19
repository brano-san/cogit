//! Debounced, filtered filesystem watching for Git repositories.
//!
//! Scope limits are INV-06: watching a repository root recursively exhausts OS handles
//! on large monorepos, and `.git/objects` alone produces thousands of events during a
//! single `fetch`.

use serde::Serialize;
use std::path::Path;

/// Debounce window. Long enough to collapse a burst from one Git command, short enough
/// that the UI still feels immediate.
pub const DEBOUNCE_MS: u64 = 100;

/// Paths inside `.git` that are worth watching, relative to the git directory.
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

/// Directory names never worth watching, regardless of where they appear.
pub const EXCLUDED_DIRS: &[&str] = &[
    "objects",
    "target",
    "build",
    "node_modules",
    "dist",
    ".venv",
    "__pycache__",
];

/// What changed, so the UI can refresh one panel instead of everything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub enum ChangeKind {
    Head,
    Index,
    Refs,
    WorkingTree,
    Stash,
    Config,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct RepoChanged {
    pub kind: ChangeKind,
    /// Repository-relative path, using `/` on every platform.
    pub path: String,
}

#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    #[error("failed to start watching {path}: {source}")]
    Start { path: String, source: notify::Error },
}

/// Whether a path should be ignored outright.
///
/// Applied before any `.gitignore` handling, because these directories are expensive
/// even to enumerate.
#[must_use]
pub fn is_excluded(path: &Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|name| EXCLUDED_DIRS.contains(&name))
    })
}

/// Classifies a path inside the git directory into a [`ChangeKind`].
///
/// Returns `None` for paths that carry no useful signal.
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
        // Both live under refs/, but they refresh different panels.
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
