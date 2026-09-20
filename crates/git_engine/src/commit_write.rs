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
        if !request.only.is_empty() {
            args.push("--only");
            args.push("--");
            args.extend(request.only.iter().map(String::as_str));
        }

        self.run_git(&args)?;
        let oid = self
            .run_git_reading(&["rev-parse", "HEAD"])?
            .stdout
            .trim()
            .to_owned();
        if request.no_verify {
            self.record_bypass(&oid, &request.message);
        }
        Ok(oid)
    }
}
