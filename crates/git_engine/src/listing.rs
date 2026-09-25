//! Every file in the repository, and what is written inside them.
//!
//! The Files panel searches what changed. That is the wrong answer to "where is this
//! file": in SmartGit the search finds a file whether or not anything happened to it, so
//! the panel needs a list that is not the change list, and a way to look inside.

use crate::{GitError, RepoHandle, Result};
use serde::Serialize;
use std::collections::BTreeSet;

/// Nothing larger is opened. A file this big is a build artefact or a dump, and reading
/// it would cost more than the answer is worth.
pub const MAX_SEARCH_BYTES: u64 = 2 * 1024 * 1024;

/// Git's own rule: a NUL byte anywhere in the first 8000 bytes means binary.
const BINARY_PREFIX: usize = 8000;

/// How much of a matching line travels to the panel. A minified bundle is one line.
pub const PREVIEW_CHARS: usize = 200;

/// Results are handed over in batches rather than one lump, so a search across a large
/// repository starts filling the panel while it is still running.
pub const BATCH: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContentMatch {
    pub path: String,
    pub line: u32,
    pub preview: String,
}

/// Which files to look inside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SearchScope {
    /// What the Files panel already shows. The default, and the cheap one.
    Changed,
    /// Every file in the repository.
    All,
}

#[derive(Debug, Clone, Copy)]
pub struct SearchRequest<'a> {
    pub query: &'a str,
    pub is_regex: bool,
    pub scope: SearchScope,
}

/// Either a compiled pattern or a plain substring. A literal query must stay literal:
/// `a.b` is a filename, not "any character between a and b".
enum Matcher {
    Literal(String),
    Pattern(regex::Regex),
}

impl Matcher {
    fn build(request: &SearchRequest<'_>) -> Result<Self> {
        if request.is_regex {
            let compiled = regex::Regex::new(request.query).map_err(|err| {
                GitError::Internal(format!("the search pattern is invalid: {err}"))
            })?;
            Ok(Self::Pattern(compiled))
        } else {
            Ok(Self::Literal(request.query.to_owned()))
        }
    }

    fn hits(&self, line: &str) -> bool {
        match self {
            Self::Literal(needle) => line.contains(needle.as_str()),
            Self::Pattern(pattern) => pattern.is_match(line),
        }
    }
}

#[must_use]
fn is_binary(data: &[u8]) -> bool {
    data.iter().take(BINARY_PREFIX).any(|byte| *byte == 0)
}

/// The matching line, trimmed of its indentation and cut to something a row can show.
#[must_use]
fn preview(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.chars().count() <= PREVIEW_CHARS {
        return trimmed.to_owned();
    }
    trimmed.chars().take(PREVIEW_CHARS).collect()
}

impl RepoHandle {
    /// Every path the repository knows about: tracked plus untracked, never ignored.
    ///
    /// Sorted and deduplicated, because a file can be both in the index and reported by
    /// the worktree walk, and a list the user searches should not show it twice.
    pub fn all_files(&self) -> Result<Vec<String>> {
        let mut paths: BTreeSet<String> = BTreeSet::new();

        if !self.repo.is_bare() {
            let index = self
                .repo
                .index()
                .map_err(|err| GitError::Internal(format!("cannot read the index: {err}")))?;
            for entry in index.entries() {
                paths.insert(entry.path(&index).to_string());
            }

            // The walk is what adds files no commit has ever seen; `UntrackedFiles::Files`
            // is what keeps it from collapsing a new directory into one entry.
            let iter = self
                .repo
                .status(gix::progress::Discard)
                .map_err(|err| GitError::Internal(format!("cannot start status: {err}")))?
                .untracked_files(gix::status::UntrackedFiles::Files)
                .into_iter(None::<gix::bstr::BString>)
                .map_err(|err| GitError::Internal(format!("cannot read status: {err}")))?;

            for item in iter {
                let item =
                    item.map_err(|err| GitError::Internal(format!("status failed: {err}")))?;
                if let gix::status::Item::IndexWorktree(worktree) = item {
                    paths.insert(worktree.rela_path().to_string());
                }
            }
        }

        Ok(paths.into_iter().collect())
    }

    /// Every file of `rev`'s tree, sorted: what a commit's file list shows as unchanged.
    ///
    /// A submodule is one entry, as in the change list; its contents are another repository.
    pub fn tree_files(&self, rev: &str) -> Result<Vec<String>> {
        let tree = self
            .find_commit(rev)?
            .tree()
            .map_err(|err| GitError::Internal(format!("cannot read the tree of {rev}: {err}")))?;
        let mut recorder = gix::traverse::tree::Recorder::default();
        tree.traverse()
            .breadthfirst(&mut recorder)
            .map_err(|err| GitError::Internal(format!("cannot walk the tree of {rev}: {err}")))?;

        let mut paths: Vec<String> = recorder
            .records
            .into_iter()
            .filter(|entry| !entry.mode.is_tree())
            .map(|entry| entry.filepath.to_string())
            .collect();
        paths.sort();
        Ok(paths)
    }

    /// Looks inside files for `query`, handing over matches in batches.
    ///
    /// `cancelled` is consulted between files rather than between lines: a file is small
    /// enough to finish, and checking per line would cost more than it saves. `on_batch`
    /// is called with at most [`BATCH`] matches at a time.
    pub fn search_contents(
        &self,
        request: &SearchRequest<'_>,
        cancelled: &dyn Fn() -> bool,
        on_batch: &mut dyn FnMut(Vec<ContentMatch>),
    ) -> Result<()> {
        let matcher = Matcher::build(request)?;

        let paths = match request.scope {
            SearchScope::All => self.all_files()?,
            SearchScope::Changed => self.changed_file_paths()?,
        };

        let mut batch: Vec<ContentMatch> = Vec::with_capacity(BATCH);

        for path in paths {
            if cancelled() {
                break;
            }

            let Some(text) = self.readable(&path) else {
                continue;
            };

            for (number, line) in text.lines().enumerate() {
                if !matcher.hits(line) {
                    continue;
                }

                batch.push(ContentMatch {
                    path: path.clone(),
                    line: u32::try_from(number + 1).unwrap_or(u32::MAX),
                    preview: preview(line),
                });

                if batch.len() >= BATCH {
                    on_batch(std::mem::take(&mut batch));
                    batch.reserve(BATCH);
                }
            }
        }

        if !batch.is_empty() {
            on_batch(batch);
        }

        Ok(())
    }

    /// The paths the Files panel already shows. A new folder is one collapsed `dir/` row
    /// there, and a folder cannot be read: its untracked files are searched instead.
    fn changed_file_paths(&self) -> Result<Vec<String>> {
        let files = self.worktree_files()?;
        let mut paths: BTreeSet<String> = BTreeSet::new();
        let mut folders: Vec<gix::bstr::BString> = Vec::new();
        for file in files.staged.iter().chain(files.unstaged.iter()) {
            if file.path.ends_with('/') {
                folders.push(file.path.as_str().into());
            } else {
                paths.insert(file.path.clone());
            }
        }
        if !folders.is_empty() {
            paths.extend(self.untracked_files_in(folders)?);
        }
        Ok(paths.into_iter().collect())
    }

    fn untracked_files_in(&self, folders: Vec<gix::bstr::BString>) -> Result<Vec<String>> {
        let iter = self
            .repo
            .status(gix::progress::Discard)
            .map_err(|err| GitError::Internal(format!("cannot start status: {err}")))?
            .untracked_files(gix::status::UntrackedFiles::Files)
            .into_iter(folders)
            .map_err(|err| GitError::Internal(format!("cannot read status: {err}")))?;

        let mut paths = Vec::new();
        for item in iter {
            let item = item.map_err(|err| GitError::Internal(format!("status failed: {err}")))?;
            if let gix::status::Item::IndexWorktree(
                gix::status::index_worktree::Item::DirectoryContents { entry, .. },
            ) = item
                && entry.status == gix::dir::entry::Status::Untracked
            {
                paths.push(entry.rela_path.to_string());
            }
        }
        Ok(paths)
    }

    /// `None` for anything not worth opening: missing, too big, or binary.
    fn readable(&self, path: &str) -> Option<String> {
        let full = self.root().join(path);

        let size = std::fs::metadata(&full).ok()?.len();
        if size > MAX_SEARCH_BYTES {
            return None;
        }

        let bytes = std::fs::read(&full).ok()?;
        if is_binary(&bytes) {
            return None;
        }

        // Lossy on purpose: a file with one bad byte is still worth searching, and the
        // alternative is telling the user their file does not exist.
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nul_byte_makes_a_file_binary() {
        assert!(is_binary(b"abc\0def"));
        assert!(!is_binary(b"abcdef"));
    }

    #[test]
    fn a_nul_past_the_prefix_is_not_looked_at() {
        let mut data = vec![b'a'; BINARY_PREFIX + 10];
        data[BINARY_PREFIX + 5] = 0;
        assert!(!is_binary(&data), "git stops looking after 8000 bytes too");
    }

    #[test]
    fn a_preview_loses_its_indentation() {
        assert_eq!(preview("    let x = 1;  "), "let x = 1;");
    }

    #[test]
    fn a_long_line_is_cut_to_the_limit() {
        let line = "x".repeat(PREVIEW_CHARS * 3);
        assert_eq!(preview(&line).chars().count(), PREVIEW_CHARS);
    }

    #[test]
    fn a_preview_counts_characters_rather_than_bytes() {
        let line = "я".repeat(PREVIEW_CHARS * 2);
        assert_eq!(preview(&line).chars().count(), PREVIEW_CHARS);
    }

    #[test]
    fn a_literal_matcher_does_not_read_its_query_as_a_pattern() {
        let matcher = Matcher::build(&SearchRequest {
            query: "a.b",
            is_regex: false,
            scope: SearchScope::All,
        })
        .expect("builds");

        assert!(matcher.hits("xxa.byy"));
        assert!(!matcher.hits("xxaxbyy"));
    }
}
