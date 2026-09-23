use crate::{AppState, Recovery, RepoId, named};
use git_engine::{GitError, IndexEditorSides, IndexFlag};
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, GitError>;

impl AppState {
    /// Where the repository lives on disk, for the desktop actions that are not Git's.
    pub fn root_of(&self, repo: RepoId) -> Result<PathBuf> {
        self.get(repo)
            .map(|open| open.root)
            .ok_or_else(|| GitError::RepoNotFound(format!("id {}", repo.0)))
    }

    pub fn remove_from_repository(
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

    pub fn move_path(&self, repo: RepoId, from: &str, to: &str) -> Result<()> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.move_path(from, to)
    }

    pub fn set_index_flag(
        &self,
        repo: RepoId,
        paths: &[String],
        flag: IndexFlag,
        on: bool,
    ) -> Result<()> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.set_index_flag(paths, flag, on)
    }

    pub fn index_editor_sides(&self, repo: RepoId, path: &str) -> Result<IndexEditorSides> {
        self.handle(repo)?.index_editor_sides(path)
    }

    /// A side left `None` was not edited and is not written.
    pub fn write_index_editor(
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

    pub fn save_blob(&self, repo: RepoId, rev: &str, path: &str, target: &Path) -> Result<()> {
        self.handle(repo)?.save_blob(rev, path, target)
    }

    /// Under the system's temporary folder, where the desktop may open it read-only.
    pub fn export_read_only(&self, repo: RepoId, rev: &str, path: &str) -> Result<PathBuf> {
        let dir = std::env::temp_dir().join("cogit-view");
        self.handle(repo)?.export_read_only(rev, path, &dir)
    }

    pub fn apply_commit_file(
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

    pub fn present_on_disk(&self, repo: RepoId, paths: &[String]) -> Result<Vec<String>> {
        Ok(self.handle(repo)?.present_on_disk(paths))
    }
}
