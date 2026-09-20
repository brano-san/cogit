use crate::{GitError, RepoHandle, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TagRequest {
    pub name: String,
    /// `None` means HEAD.
    pub target: Option<String>,
    /// A message makes the tag annotated, which is what a release wants.
    pub message: Option<String>,
    pub force: bool,
}

impl RepoHandle {
    pub fn create_tag(&self, request: &TagRequest) -> Result<()> {
        let name = require(&request.name)?;

        let mut args = vec!["tag"];
        if request.force {
            args.push("--force");
        }
        if let Some(message) = &request.message {
            args.push("--annotate");
            args.push("--message");
            args.push(message);
        }
        args.push(name);
        if let Some(target) = &request.target {
            args.push(target);
        }
        self.run_git(&args).map(drop)
    }

    pub fn delete_tag(&self, name: &str) -> Result<()> {
        let name = require(name)?;
        self.run_git(&["tag", "--delete", name]).map(drop)
    }
}

fn require(name: &str) -> Result<&str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(GitError::InvalidState("an empty tag name".to_owned()));
    }
    Ok(trimmed)
}
