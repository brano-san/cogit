use crate::{AppState, Recovery, RepoId, backup_failed, named};
use git_engine::{GitError, IndexEditorSides, IndexFlag};
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, GitError>;

impl AppState {
    /// Where the repository lives on disk, for the desktop actions that are not Git's.
    pub fn root_of(&self, repo: RepoId) -> Result<PathBuf> {
        self.get(repo)
            .map(|open| open.root)
            .ok_or_else(|| crate::not_open(repo))
    }

    /// Delete, to the bin (`trash`). The files are kept in the object store first, so Undo
    /// writes them back whatever became of the bin; a folder is left to the bin (F-071).
    pub fn move_to_trash(
        &self,
        repo: RepoId,
        paths: &[String],
        trash: impl FnOnce(&[PathBuf]) -> std::io::Result<()>,
    ) -> Result<()> {
        if let Some(remote) = self.remote(repo) {
            return remote.ask(
                &crate::backend::Request::Trash {
                    repo: remote.id(),
                    paths: paths.to_vec(),
                },
                None,
            );
        }
        let _quiet = self.quiet(repo);
        let root = self.root_of(repo)?;
        let absolute: Vec<PathBuf> = paths.iter().map(|path| root.join(path)).collect();
        let recovery = {
            let kept = self
                .handle(repo)?
                .keep_files(paths)
                .map_err(|err| backup_failed("deleting", &err))?;
            Recovery::Files {
                kept: paths.iter().cloned().zip(kept).collect(),
            }
        };
        trash(&absolute).map_err(|err| GitError::Io(err.to_string()))?;
        self.record(repo, format!("Delete {}", named(paths)), recovery);
        Ok(())
    }

    pub(crate) fn local_remove_from_repository(
        &self,
        repo: RepoId,
        paths: &[String],
        delete_local: bool,
    ) -> Result<()> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?
            .remove_from_repository(paths, delete_local)?;
        let what = if delete_local {
            "Delete"
        } else {
            "Stop tracking"
        };
        self.record(repo, format!("{what} {}", named(paths)), Recovery::None);
        Ok(())
    }

    pub(crate) fn local_move_path(&self, repo: RepoId, from: &str, to: &str) -> Result<()> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.move_path(from, to)
    }

    pub(crate) fn local_set_index_flag(
        &self,
        repo: RepoId,
        paths: &[String],
        flag: IndexFlag,
        on: bool,
    ) -> Result<()> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.set_index_flag(paths, flag, on)
    }

    /// The working file for the diff view's editor, or why it cannot be edited (F-722).
    pub(crate) fn local_read_editable(
        &self,
        repo: RepoId,
        spec: &git_engine::DiffSpec,
        path: &str,
    ) -> Result<git_engine::editable::EditableFile> {
        self.handle(repo)?.read_editable(spec, path)
    }

    /// Not quiet: the watcher's refresh is what re-reads the diff after a save.
    pub(crate) fn local_save_editable(
        &self,
        repo: RepoId,
        path: &str,
        text: &str,
        shape: git_engine::editable::Shape,
        stamp: &str,
        force: bool,
    ) -> Result<git_engine::editable::SaveOutcome> {
        self.handle(repo)?
            .save_editable(path, text, shape, stamp, force)
    }

    pub(crate) fn local_index_editor_sides(
        &self,
        repo: RepoId,
        path: &str,
    ) -> Result<IndexEditorSides> {
        self.handle(repo)?.index_editor_sides(path)
    }

    /// A side left `None` was not edited and is not written.
    pub(crate) fn local_write_index_editor(
        &self,
        repo: RepoId,
        path: &str,
        index: Option<&str>,
        worktree: Option<&str>,
    ) -> Result<()> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        if let Some(text) = index {
            handle.write_index_text(path, text)?;
        }
        if let Some(text) = worktree {
            handle.write_worktree_text(path, text)?;
        }
        Ok(())
    }

    pub(crate) fn local_save_blob(
        &self,
        repo: RepoId,
        rev: &str,
        path: &str,
        target: &Path,
    ) -> Result<()> {
        self.handle(repo)?.save_blob(rev, path, target)
    }

    /// Under `dir`, a folder of the current user only (never the shared temp folder).
    pub(crate) fn local_export_read_only(
        &self,
        repo: RepoId,
        rev: &str,
        path: &str,
        dir: &Path,
    ) -> Result<PathBuf> {
        self.handle(repo)?.export_read_only(rev, path, dir)
    }

    pub(crate) fn local_apply_commit_file(
        &self,
        repo: RepoId,
        rev: &str,
        path: &str,
        old_path: Option<&str>,
        reverse: bool,
    ) -> Result<()> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?
            .apply_commit_file(rev, path, old_path, reverse)
    }

    pub(crate) fn local_present_on_disk(
        &self,
        repo: RepoId,
        paths: &[String],
    ) -> Result<Vec<String>> {
        Ok(self.handle(repo)?.present_on_disk(paths))
    }
}
