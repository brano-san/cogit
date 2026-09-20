use crate::{GitError, RepoHandle, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MergeOptions {
    pub source: String,
    pub no_fast_forward: bool,
    pub squash: bool,
    pub message: Option<String>,
}

impl RepoHandle {
    pub fn merge(&self, options: &MergeOptions) -> Result<()> {
        let source = options.source.trim();
        if source.is_empty() {
            return Err(GitError::InvalidState("nothing to merge".to_owned()));
        }

        let mut args = vec!["merge"];
        if options.squash {
            args.push("--squash");
        } else if options.no_fast_forward {
            args.push("--no-ff");
        }
        if let Some(message) = &options.message {
            args.push("--message");
            args.push(message);
        } else {
            // The message comes from Cogit's own UI, never from a terminal editor (R-26).
            args.push("--no-edit");
        }
        args.push(source);

        self.run_git(&args).map(drop)
    }
}
