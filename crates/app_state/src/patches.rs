//! Patches (F-948): routed calls run by the repository, WSL too; files are written here.

use crate::safety::Recovery;
use crate::{AppState, RepoId, named};
use git_engine::{GitError, PatchFile, PatchMode, PatchPreview};
use std::path::Path;

impl AppState {
    pub(crate) fn local_format_patches(
        &self,
        repo: RepoId,
        commits: &[String],
    ) -> Result<Vec<PatchFile>, GitError> {
        self.handle(repo)?.format_patches(commits)
    }

    pub(crate) fn local_worktree_patch(
        &self,
        repo: RepoId,
        paths: &[String],
        staged: bool,
    ) -> Result<Vec<u8>, GitError> {
        self.handle(repo)?.worktree_patch(paths, staged)
    }

    pub(crate) fn local_patch_preview(
        &self,
        repo: RepoId,
        text: &str,
        mode: PatchMode,
    ) -> Result<PatchPreview, GitError> {
        self.handle(repo)?.patch_preview(text, mode)
    }

    /// A plain diff is undone in reverse; `am` as a move, also when it stopped midway.
    pub(crate) fn local_apply_user_patch(
        &self,
        repo: RepoId,
        text: &str,
        mode: PatchMode,
    ) -> Result<(), GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        if mode == PatchMode::Mailbox {
            let before = crate::safety::Before::head(&handle)?;
            let result = handle.apply_user_patch(text, mode);
            self.record_move(
                repo,
                &handle,
                before,
                "Apply patches (am)".to_owned(),
                &result,
            );
            return result;
        }
        let files: Vec<String> = handle
            .patch_preview(text, mode)
            .map(|preview| preview.files.into_iter().map(|file| file.path).collect())
            .unwrap_or_default();
        let result = handle.apply_user_patch(text, mode);
        let place = if mode == PatchMode::Index {
            "the index"
        } else {
            "the working tree"
        };
        let what = format!("Apply a patch of {} to {place}", named(&files));
        match &result {
            Ok(()) => self.record(
                repo,
                what,
                Recovery::Patch {
                    path: files.join(", "),
                    patch: text.to_owned(),
                    reverse: true,
                    index: mode == PatchMode::Index,
                },
            ),
            // Written with conflict markers, which a reverse apply would only fail on.
            Err(_) if !handle.conflicted_paths().unwrap_or_default().is_empty() => {
                self.record(repo, what + " (with conflicts)", Recovery::None);
            }
            Err(_) => {}
        }
        result
    }

    /// One file each into the folder `target`, or all in the file `target`.
    pub fn save_commit_patches(
        &self,
        repo: RepoId,
        commits: &[String],
        target: &Path,
        one_file: bool,
    ) -> Result<Vec<String>, GitError> {
        let from_ssh = matches!(
            crate::location::RepoLocation::of(&self.root_of(repo)?),
            crate::location::RepoLocation::Ssh { .. }
        );
        let files = self.format_patches(repo, commits)?;
        if one_file {
            let bytes: Vec<u8> = files.into_iter().flat_map(|file| file.bytes).collect();
            std::fs::write(target, bytes)?;
            return Ok(vec![git_engine::slash_path(target)]);
        }
        std::fs::create_dir_all(target)?;
        files
            .into_iter()
            .map(|file| {
                let path = if from_ssh {
                    file.write_to(target)?
                } else {
                    let path = target.join(file.name);
                    std::fs::write(&path, file.bytes)?;
                    path
                };
                Ok(git_engine::slash_path(&path))
            })
            .collect()
    }

    pub fn commit_patch_text(&self, repo: RepoId, commits: &[String]) -> Result<String, GitError> {
        let bytes: Vec<u8> = self
            .format_patches(repo, commits)?
            .into_iter()
            .flat_map(|file| file.bytes)
            .collect();
        Ok(String::from_utf8_lossy_owned(bytes))
    }

    pub fn save_worktree_patch(
        &self,
        repo: RepoId,
        paths: &[String],
        staged: bool,
        target: &Path,
    ) -> Result<(), GitError> {
        let bytes = self.worktree_patch(repo, paths, staged)?;
        std::fs::write(target, bytes)?;
        Ok(())
    }

    pub fn worktree_patch_text(
        &self,
        repo: RepoId,
        paths: &[String],
        staged: bool,
    ) -> Result<String, GitError> {
        Ok(String::from_utf8_lossy_owned(
            self.worktree_patch(repo, paths, staged)?,
        ))
    }
}

/// Not UTF-8 is refused: lossy text would apply altered.
pub fn read_patch_file(path: &Path) -> Result<String, GitError> {
    let bytes = std::fs::read(path)?;
    String::from_utf8(bytes).map_err(|_| {
        GitError::InvalidState(format!(
            "{} is not UTF-8 text; apply it with git in a terminal",
            path.display()
        ))
    })
}
