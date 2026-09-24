//! The Files panel's context menus beyond staging (#40, #41).

use crate::{FileStatus, GitError, Head, RepoHandle, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum IndexFlag {
    AssumeUnchanged,
    SkipWorktree,
}

/// `None` where the file is absent; every side `None` when any of them is not text.
#[derive(Debug, Clone, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IndexEditorSides {
    pub head: Option<String>,
    pub index: Option<String>,
    pub worktree: Option<String>,
    pub binary: bool,
}

fn as_text(bytes: Option<Vec<u8>>) -> std::result::Result<Option<String>, ()> {
    match bytes {
        None => Ok(None),
        Some(bytes) if bytes.contains(&0) => Err(()),
        Some(bytes) => String::from_utf8(bytes).map(Some).map_err(drop),
    }
}

impl RepoHandle {
    /// `git rm --cached` keeps the files on disk, untracked; `git rm` deletes them too.
    pub fn remove_from_repository(&self, paths: &[String], delete_local: bool) -> Result<()> {
        crate::staging::require_paths(paths)?;
        let args: &[&str] = if delete_local {
            &["rm", "-r"]
        } else {
            &["rm", "--cached", "-r"]
        };
        self.run_git_paths(args, paths).map(drop)
    }

    /// `git mv` when tracked, a move on disk otherwise; never over an existing file.
    pub fn move_path(&self, from: &str, to: &str) -> Result<()> {
        let (from, to) = (from.trim_end_matches('/'), to.trim_end_matches('/'));
        if from == to {
            return Ok(());
        }
        let target = self.root().join(to);
        if std::fs::symlink_metadata(&target).is_ok() {
            return Err(GitError::InvalidState(format!("{to} already exists")));
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if self.tracks(from)? {
            return self.run_git(&["mv", "--", from, to]).map(drop);
        }
        std::fs::rename(self.root().join(from), target)?;
        Ok(())
    }

    pub fn set_index_flag(&self, paths: &[String], flag: IndexFlag, on: bool) -> Result<()> {
        crate::staging::require_paths(paths)?;
        let option = match (flag, on) {
            (IndexFlag::AssumeUnchanged, true) => "--assume-unchanged",
            (IndexFlag::AssumeUnchanged, false) => "--no-assume-unchanged",
            (IndexFlag::SkipWorktree, true) => "--skip-worktree",
            (IndexFlag::SkipWorktree, false) => "--no-skip-worktree",
        };
        // `update-index` has no `--pathspec-from-file`.
        for batch in crate::runner::command_line_batches(paths) {
            let mut args = vec!["update-index", option, "--"];
            args.extend(batch.iter().map(String::as_str));
            self.run_git(&args)?;
        }
        Ok(())
    }

    pub fn index_editor_sides(&self, path: &str) -> Result<IndexEditorSides> {
        let head = match self.head()? {
            Head::Unborn { .. } => None,
            _ => self.blob_at("HEAD", path)?,
        };
        let sides = (
            as_text(head),
            as_text(self.blob_in_index(path)?),
            as_text(self.blob_on_disk(path)?),
        );
        match sides {
            (Ok(head), Ok(index), Ok(worktree)) => Ok(IndexEditorSides {
                head,
                index,
                worktree,
                binary: false,
            }),
            _ => Ok(IndexEditorSides {
                binary: true,
                ..IndexEditorSides::default()
            }),
        }
    }

    /// Exactly `text`, no filters: the editor shows the index as it is. Mode is kept.
    pub fn write_index_text(&self, path: &str, text: &str) -> Result<()> {
        let hashed = self.run_git_fed(
            &["hash-object", "-w", "--no-filters", "--stdin"],
            text.as_bytes(),
        )?;
        let oid = hashed.stdout.trim();
        let mode = self.index_mode(path).unwrap_or("100644");
        let entry = format!("{mode},{oid},{path}");
        self.run_git(&["update-index", "--add", "--cacheinfo", &entry])
            .map(drop)
    }

    pub fn write_worktree_text(&self, path: &str, text: &str) -> Result<()> {
        if self.is_bare() {
            return Err(GitError::InvalidState(
                "a bare repository has no working tree".to_owned(),
            ));
        }
        std::fs::write(self.root().join(path), text)?;
        Ok(())
    }

    fn index_mode(&self, path: &str) -> Option<&'static str> {
        use gix::index::entry::Mode;
        let index = self.repo.index_or_empty().ok()?;
        let mode = index.entry_by_path(path.into())?.mode;
        Some(if mode == Mode::FILE_EXECUTABLE {
            "100755"
        } else if mode == Mode::SYMLINK {
            "120000"
        } else {
            "100644"
        })
    }

    fn blob_or_refuse(&self, rev: &str, path: &str) -> Result<Vec<u8>> {
        self.blob_at(rev, path)?
            .ok_or_else(|| GitError::InvalidState(format!("{path} is not in {rev}")))
    }

    pub fn save_blob(&self, rev: &str, path: &str, target: &Path) -> Result<()> {
        let bytes = self.blob_or_refuse(rev, path)?;
        std::fs::write(target, bytes)?;
        Ok(())
    }

    /// Under `dir/<commit>/<path>`; a commit's file never changes, so a copy is reused.
    pub fn export_read_only(&self, rev: &str, path: &str, dir: &Path) -> Result<PathBuf> {
        let commit = self
            .repo
            .rev_parse_single(rev)
            .map_err(|err| GitError::InvalidState(format!("cannot resolve {rev}: {err}")))?
            .to_string();
        let target = dir.join(&commit[..commit.len().min(12)]).join(path);
        if target.is_file() {
            return Ok(target);
        }
        let bytes = self.blob_or_refuse(rev, path)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, bytes)?;
        let mut permissions = std::fs::metadata(&target)?.permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&target, permissions)?;
        Ok(target)
    }

    /// Cherry-picks (or with `reverse`, reverts) what `rev` did to one file, against its
    /// first parent. `--3way` falls back to a merge, leaving conflict markers when it must.
    pub fn apply_commit_file(
        &self,
        rev: &str,
        path: &str,
        old_path: Option<&str>,
        reverse: bool,
    ) -> Result<()> {
        let mut paths = Vec::with_capacity(2);
        if let Some(old) = old_path.filter(|old| *old != path) {
            paths.push(old);
        }
        paths.push(path);

        let patch = match self.first_parent(rev)? {
            // Plumbing, like the root case below: porcelain `git diff` follows the user's
            // diff.noprefix, color.ui and diff.external, and `git apply` cannot read that.
            Some(parent) => {
                let mut args = vec![
                    "diff-tree",
                    "-p",
                    "--binary",
                    "--full-index",
                    "-M",
                    &parent,
                    rev,
                    "--",
                ];
                args.extend(&paths);
                self.run_git_bytes_literal(&args)?
            }
            None => {
                let mut args = vec![
                    "diff-tree",
                    "-p",
                    "--binary",
                    "--full-index",
                    "--root",
                    "--no-commit-id",
                    "-M",
                    rev,
                    "--",
                ];
                args.extend(&paths);
                self.run_git_bytes_literal(&args)?
            }
        };
        if patch.iter().all(u8::is_ascii_whitespace) {
            return Err(GitError::InvalidState(format!(
                "{path} did not change in {rev}"
            )));
        }

        let mut args = vec!["apply", "--3way", "--whitespace=nowarn"];
        if reverse {
            args.push("--reverse");
        }
        args.push("-");
        self.run_git_fed(&args, &patch).map(drop)
    }

    #[must_use]
    pub fn present_on_disk(&self, paths: &[String]) -> Vec<String> {
        paths
            .iter()
            .filter(|path| std::fs::symlink_metadata(self.root().join(path)).is_ok())
            .cloned()
            .collect()
    }
}

impl RepoHandle {
    /// Appends to `.gitignore`, skipping patterns it already contains. The file is read as
    /// bytes and only ever added to: one in another encoding keeps every line it had.
    pub fn add_to_gitignore(&self, paths: &[String]) -> Result<()> {
        if paths.is_empty() {
            return Err(GitError::InvalidState("no paths to ignore".to_owned()));
        }

        let file = self.root().join(".gitignore");
        let existing = match std::fs::read(&file) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(err) => return Err(err.into()),
        };
        let text = String::from_utf8_lossy(&existing);
        let known: std::collections::HashSet<&str> = text.lines().map(str::trim).collect();

        let mut added = String::new();
        for path in paths {
            if path.trim().is_empty() {
                continue;
            }
            let pattern = ignore_pattern(path);
            if known.contains(pattern.as_str()) || added.lines().any(|line| line == pattern) {
                continue;
            }
            added.push_str(&pattern);
            added.push('\n');
        }
        if added.is_empty() {
            return Ok(());
        }

        if !existing.is_empty() && !existing.ends_with(b"\n") {
            added.insert(0, '\n');
        }
        let mut out = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file)?;
        std::io::Write::write_all(&mut out, added.as_bytes())?;
        Ok(())
    }

    /// Only untracked paths: deleting a tracked one is `discard`, which keeps a stash.
    pub fn delete_untracked(&self, paths: &[String]) -> Result<()> {
        if paths.is_empty() {
            return Err(GitError::InvalidState("no paths to delete".to_owned()));
        }

        let untracked: std::collections::HashSet<String> = self
            .worktree_files()?
            .unstaged
            .into_iter()
            .filter(|entry| entry.status == FileStatus::Untracked)
            .map(|entry| entry.path)
            .collect();

        for path in paths {
            if !untracked.contains(path) {
                return Err(GitError::InvalidState(format!(
                    "{path} is tracked; use discard instead"
                )));
            }
        }

        for path in paths {
            let target = self.root().join(path.trim_end_matches('/'));
            let result = if target.is_dir() {
                std::fs::remove_dir_all(&target)
            } else {
                std::fs::remove_file(&target)
            };
            result?;
        }
        Ok(())
    }
}

/// The one path the user picked, as a `.gitignore` line: anchored to the top, so a file of
/// the same name in a folder stays visible, and with the characters git reads as a pattern
/// escaped, so `test[1].txt` is not a character class matching `test1.txt`.
fn ignore_pattern(path: &str) -> String {
    let mut pattern = String::from("/");
    for c in path.chars() {
        if matches!(c, '\\' | '[' | ']' | '*' | '?') {
            pattern.push('\\');
        }
        pattern.push(c);
    }
    // Trailing spaces are dropped from a pattern unless escaped.
    let body = pattern.trim_end_matches(' ');
    let spaces = pattern.len() - body.len();
    let mut out = body.to_owned();
    for _ in 0..spaces {
        out.push_str("\\ ");
    }
    out
}
