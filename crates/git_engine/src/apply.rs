use crate::{GitError, RepoHandle, Result};

/// What a patch is applied to. Staging never touches the file on disk; Discard only
/// touches the file on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchTarget {
    /// `--cached`: the index alone, which is what Stage and Unstage change.
    Index,
    /// The working tree, which is where Discard throws the lines away.
    WorkTree,
}

impl RepoHandle {
    /// Applies a synthetic patch to the index only. The full patch text goes to the log
    /// **before** it runs: a patch that corrupts a file is unrecoverable otherwise (R-04).
    pub fn apply_patch(&self, patch: &str, reverse: bool) -> Result<()> {
        self.apply_patch_to(patch, reverse, PatchTarget::Index)
    }

    pub fn apply_patch_to(&self, patch: &str, reverse: bool, target: PatchTarget) -> Result<()> {
        if patch.trim().is_empty() {
            return Err(GitError::InvalidState("an empty patch".to_owned()));
        }

        let mut args = vec!["apply", "--unidiff-zero", "--whitespace=nowarn"];
        if target == PatchTarget::Index {
            args.insert(1, "--cached");
        }
        if reverse {
            args.push("--reverse");
        }
        args.push("-");

        tracing::info!(patch = %patch, "applying a patch");
        self.run_git_fed(&args, patch.as_bytes())
            .map(drop)
            .inspect_err(
                |err| tracing::error!(patch = %patch, error = ?err, "the patch did not apply"),
            )
    }
}
