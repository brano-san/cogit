use crate::{GitError, RepoHandle, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CommitRequest {
    pub message: String,
    pub amend: bool,
    pub no_verify: bool,
    /// Empty means everything staged; a list narrows the commit to those paths (T6.8).
    #[serde(default)]
    pub only: Vec<String>,
}

impl RepoHandle {
    pub fn commit(&self, request: &CommitRequest) -> Result<String> {
        if request.message.trim().is_empty() {
            return Err(GitError::InvalidState(
                "a commit message cannot be empty".to_owned(),
            ));
        }

        let mut args = vec!["commit"];
        if request.amend {
            args.push("--amend");
        }
        if request.no_verify {
            args.push("--no-verify");
        }
        // `-m` takes the next argument verbatim, so a message starting with `--` is safe.
        args.push("-m");
        args.push(&request.message);
        if request.only.is_empty() {
            self.run_git(&args)?;
        } else {
            args.push("--only");
            self.run_git_paths(&args, &request.only)?;
        }
        // Through gix: `rev-parse HEAD` was a second process (R-313).
        let oid = self
            .repo
            .head_id()
            .map_err(|err| GitError::Internal(format!("cannot read the new HEAD: {err}")))?
            .to_string();
        if request.no_verify {
            self.record_bypass(&oid, &request.message);
        }
        Ok(oid)
    }
}

impl RepoHandle {
    /// `commit.template` as text, or `None` when it is unset or points nowhere.
    pub fn commit_template(&self) -> Result<Option<String>> {
        let Some(path) = self.pathname_setting("commit.template") else {
            return Ok(None);
        };
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root().join(path)
        };
        Ok(std::fs::read_to_string(resolved).ok())
    }
}
