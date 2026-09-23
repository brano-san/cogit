use crate::runner::bare_git;
use crate::{GitError, RepoHandle, Result};

#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LfsOp {
    /// `--local`: the filters go into this repository's config, not the user's.
    Install,
    Track {
        pattern: String,
    },
    Lock {
        paths: Vec<String>,
    },
    Unlock {
        paths: Vec<String>,
    },
    Prune,
}

/// `git lfs version`, or `None` when git has no `lfs` command to run.
#[must_use]
pub fn lfs_version() -> Option<String> {
    let output = bare_git(&["lfs", "version"]).ok()?;
    lfs_version_from(output.exit_code, &output.stdout)
}

#[must_use]
pub fn lfs_version_from(exit_code: Option<i32>, stdout: &str) -> Option<String> {
    let line = stdout.lines().next()?.trim();
    (exit_code == Some(0) && !line.is_empty()).then(|| line.to_owned())
}

impl RepoHandle {
    pub fn lfs_op(&self, op: &LfsOp) -> Result<()> {
        match op {
            LfsOp::Install => self.run_git(&["lfs", "install", "--local"]).map(drop),
            LfsOp::Track { pattern } => {
                let pattern = pattern.trim();
                if pattern.is_empty() || pattern.starts_with('-') {
                    return Err(GitError::InvalidState(format!(
                        "\"{pattern}\" is not a file pattern"
                    )));
                }
                self.run_git(&["lfs", "track", "--", pattern]).map(drop)
            }
            LfsOp::Lock { paths } => self.lfs_each("lock", paths),
            LfsOp::Unlock { paths } => self.lfs_each("unlock", paths),
            LfsOp::Prune => self.run_git(&["lfs", "prune"]).map(drop),
        }
    }

    /// One call per file: `git lfs lock` takes a single path.
    fn lfs_each(&self, command: &str, paths: &[String]) -> Result<()> {
        if paths.is_empty() {
            return Err(GitError::InvalidState(format!(
                "choose a file to {command}"
            )));
        }
        paths
            .iter()
            .try_for_each(|path| self.run_git(&["lfs", command, "--", path]).map(drop))
    }
}
