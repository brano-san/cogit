//! Debounced, filtered filesystem watching for Git repositories.
//!
//! Scope limits are INV-06: watching a repository root recursively exhausts OS handles
//! on large monorepos, and `.git/objects` alone produces thousands of events during a
//! single `fetch`.

mod watcher;

pub use watcher::{DEFAULT_QUIET, QuietHold, RepoWatcher};

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

const EXCLUDED_DIRS: &[&str] = &[
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
    Hooks,
    /// `.mailmap` at the root of the working tree: names and addresses to read again.
    Mailmap,
}

/// What changed; the watcher says only the kind, which is all anything reads.
#[derive(Debug, Clone)]
pub struct RepoChanged {
    pub kind: ChangeKind,
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

/// Inside the `.git` folder of a repository nested in the working tree — a submodule cloned
/// in place, a clone inside the tree — only HEAD and refs moving change what the parent
/// shows. Its index, logs, caches and lock files are rewritten by every client that looks
/// at it, the case probe of the health check included (doc/12-risks.md, R-591).
#[must_use]
pub fn is_nested_git_noise(relative: &Path) -> bool {
    let mut parts = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy());
    if !parts.any(|name| name == ".git") {
        return false;
    }
    !matches!(
        parts.next().as_deref(),
        Some("HEAD" | "packed-refs" | "refs")
    )
}

#[must_use]
pub fn classify_git_path(relative: &str) -> Option<ChangeKind> {
    let normalized = relative.replace('\\', "/");
    let normalized = normalized.trim_start_matches('/');

    if normalized == "HEAD" || normalized == "ORIG_HEAD" {
        return Some(ChangeKind::Head);
    }
    // A lock left behind by a git that died is a banner to show, and to drop once deleted.
    if normalized == "index" || normalized == "index.lock" {
        return Some(ChangeKind::Index);
    }
    if normalized == "config" {
        return Some(ChangeKind::Config);
    }
    if normalized == "refs/stash" || normalized.starts_with("logs/refs/stash") {
        return Some(ChangeKind::Stash);
    }
    if normalized == "hooks" || normalized.starts_with("hooks/") {
        return Some(ChangeKind::Hooks);
    }
    if normalized.starts_with("refs/") || normalized == "packed-refs" {
        return Some(ChangeKind::Refs);
    }
    if normalized.starts_with("MERGE_HEAD")
        || normalized.starts_with("CHERRY_PICK_HEAD")
        || normalized.starts_with("REVERT_HEAD")
        || normalized.starts_with("rebase-merge")
        || normalized.starts_with("rebase-apply")
        || normalized.starts_with("BISECT_")
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
    fn a_bisect_step_reports_as_a_head_change() {
        for name in ["BISECT_LOG", "BISECT_START", "BISECT_TERMS"] {
            assert_eq!(classify_git_path(name), Some(ChangeKind::Head), "{name}");
        }
        assert_eq!(classify_git_path("refs/bisect/bad"), Some(ChangeKind::Refs));
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

#[cfg(test)]
mod hook_tests {
    use super::*;

    #[test]
    fn a_hook_file_is_its_own_kind() {
        assert_eq!(
            classify_git_path("hooks/pre-commit"),
            Some(ChangeKind::Hooks)
        );
    }

    #[test]
    fn a_sample_is_a_hook_change_too_since_renaming_it_installs_it() {
        assert_eq!(
            classify_git_path("hooks/pre-push.sample"),
            Some(ChangeKind::Hooks)
        );
    }

    #[test]
    fn the_hooks_directory_itself_counts() {
        assert_eq!(classify_git_path("hooks"), Some(ChangeKind::Hooks));
    }

    #[test]
    fn a_backslash_path_from_windows_is_recognised() {
        assert_eq!(
            classify_git_path(r"hooks\pre-commit"),
            Some(ChangeKind::Hooks)
        );
    }

    #[test]
    fn a_path_that_merely_starts_with_the_word_is_not_a_hook() {
        assert_eq!(classify_git_path("hooksomething"), None);
    }
}
