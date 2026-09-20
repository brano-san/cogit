use crate::{GitError, RepoHandle, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CommitRequest {
    pub message: String,
    pub amend: bool,
    pub no_verify: bool,
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

        self.run_git(&args)?;
        Ok(self
            .run_git_reading(&["rev-parse", "HEAD"])?
            .stdout
            .trim()
            .to_owned())
    }
}
