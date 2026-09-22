//! A submodule opened at its own path, never discovered upward (doc/12-risks.md, R-149).

use crate::repo::env_free;
use crate::{GitError, RepoHandle, Result};
use serde::Serialize;
use std::path::Path;

/// Why a submodule path is not a repository of its own.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, specta::Type)]
#[serde(tag = "reason", rename_all = "camelCase")]
pub enum ModuleProblem {
    #[error("Directory does not exist: {path}")]
    Missing { path: String },

    #[error("Submodule is not initialized: {path}")]
    NotInitialised { path: String },

    /// `foreign`: an absolute path in another operating system's form.
    #[error("{}", dangling_message(path, target, *foreign))]
    DanglingGitFile {
        path: String,
        target: String,
        foreign: bool,
    },

    #[error("Not a Git repository: {path}: {detail}")]
    NotARepository { path: String, detail: String },
}

fn dangling_message(path: &str, target: &str, foreign: bool) -> String {
    let base = format!("The .git file of {path} points to {target}");
    if foreign {
        format!(
            "{base}, which does not exist on this system (the path was written by another operating system)"
        )
    } else {
        format!("{base}, which does not exist")
    }
}

/// On Windows `/home/user` resolves against the current drive: `E:\home\user`.
#[must_use]
pub fn is_foreign_path(target: &str) -> bool {
    if cfg!(windows) {
        target.starts_with('/') && !target.starts_with("//")
    } else {
        let bytes = target.as_bytes();
        bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && (bytes[2] == b'/' || bytes[2] == b'\\')
    }
}

pub(crate) fn gitdir_of(dot_git: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dot_git).ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix("gitdir:"))
        .map(|target| target.trim().to_owned())
}

impl RepoHandle {
    /// Opens the repository whose working tree is exactly `path`, never one above it.
    pub fn open_exact(path: &Path) -> Result<Self> {
        let shown = path.display().to_string();
        if !path.is_dir() {
            return Err(GitError::ModuleUnavailable(ModuleProblem::Missing {
                path: shown,
            }));
        }

        let dot_git = path.join(".git");
        if dot_git.is_file() {
            if let Some(target) = gitdir_of(&dot_git)
                && !path.join(&target).is_dir()
            {
                return Err(GitError::ModuleUnavailable(
                    ModuleProblem::DanglingGitFile {
                        foreign: is_foreign_path(&target),
                        path: shown,
                        target,
                    },
                ));
            }
        } else if !dot_git.exists() {
            return Err(GitError::ModuleUnavailable(ModuleProblem::NotInitialised {
                path: shown,
            }));
        }

        let repo = gix::open_opts(path, env_free()).map_err(|err| {
            GitError::ModuleUnavailable(ModuleProblem::NotARepository {
                path: shown.clone(),
                detail: err.to_string(),
            })
        })?;
        Ok(Self::from_repo(repo))
    }
}
