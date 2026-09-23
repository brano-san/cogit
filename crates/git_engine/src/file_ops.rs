//! What the file context menus of the Files panel do beyond staging: Remove, Move or
//! Rename, the two index flags, the Index Editor, and one file's change out of a commit.

use crate::{GitError, Head, RepoHandle, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The two `git update-index` switches that make Git stop looking at a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum IndexFlag {
    AssumeUnchanged,
    SkipWorktree,
}

/// One file as HEAD, the index and the disk hold it. A side is `None` where the file is
/// absent; all are `None` when any of them is not text, which the editor cannot show.
#[derive(Debug, Clone, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IndexEditorSides {
    pub head: Option<String>,
    pub index: Option<String>,
    pub worktree: Option<String>,
    pub binary: bool,
}

fn require(paths: &[String]) -> Result<()> {
    if paths.is_empty() {
        return Err(GitError::InvalidState(
            "no paths given; refusing to act on the whole repository".to_owned(),
        ));
    }
    Ok(())
}

/// `Err(())` when the bytes are not text.
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
        require(paths)?;
        let args: &[&str] = if delete_local {
            &["rm", "-r"]
        } else {
            &["rm", "--cached", "-r"]
        };
        self.run_git_paths(args, paths).map(drop)
    }

    /// A tracked file moves with `git mv`, so the rename is staged; an untracked one is
    /// only moved on disk. An existing target is never overwritten.
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
        let tracked = self
            .run_git_reading(&["ls-files", "--error-unmatch", "--", from])
            .is_ok();
        if tracked {
            return self.run_git(&["mv", "--", from, to]).map(drop);
        }
        std::fs::rename(self.root().join(from), target)?;
        Ok(())
    }

    pub fn set_index_flag(&self, paths: &[String], flag: IndexFlag, on: bool) -> Result<()> {
        require(paths)?;
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
            as_text(self.blob_on_disk(path)),
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

    /// Stages exactly `text`, with no clean filter or line-ending conversion: the editor
    /// shows the index as it is, so it writes it back as it is. The file mode is kept.
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

    /// A read-only copy under `dir/<commit>/<path>`. A commit's file never changes, so a
    /// copy already there is handed back as it is.
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
            Some(parent) => {
                let mut args = vec!["diff", "--binary", "--full-index", "-M", &parent, rev, "--"];
                args.extend(&paths);
                self.run_git_bytes(&args)?
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
                self.run_git_bytes(&args)?
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

    /// Which of `paths` exist in the working tree right now.
    #[must_use]
    pub fn present_on_disk(&self, paths: &[String]) -> Vec<String> {
        paths
            .iter()
            .filter(|path| std::fs::symlink_metadata(self.root().join(path)).is_ok())
            .cloned()
            .collect()
    }
}
