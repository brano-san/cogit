use crate::{GitError, RepoHandle, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MergeOptions {
    pub source: String,
    pub no_fast_forward: bool,
    pub squash: bool,
    pub message: Option<String>,
}

/// Several heads merged into HEAD at once: one commit with a parent per source.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OctopusOptions {
    pub sources: Vec<String>,
    pub no_fast_forward: bool,
    pub message: Option<String>,
}

/// Git's octopus strategy refuses a merge that conflicts and puts the tree back.
const OCTOPUS_FAILED: &str = "Merge with strategy octopus failed";

const OCTOPUS_SUMMARY: &str = "An octopus merge cannot stop on conflicts, so Git left everything                                as it was. Merge the branches one by one.";

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
        args.extend(["--end-of-options", source]);

        self.run_git(&args).map(drop)
    }

    /// `git merge a b c`. A conflict is refused by Git as a whole; the error says so above
    /// Git's own output.
    pub fn merge_octopus(&self, options: &OctopusOptions) -> Result<()> {
        let sources: Vec<&str> = options
            .sources
            .iter()
            .map(|source| source.trim())
            .filter(|source| !source.is_empty())
            .collect();
        if sources.len() < 2 {
            return Err(GitError::InvalidState(
                "an octopus merge needs at least two branches".to_owned(),
            ));
        }

        let mut args = vec!["merge"];
        if options.no_fast_forward {
            args.push("--no-ff");
        }
        if let Some(message) = &options.message {
            args.push("--message");
            args.push(message);
        } else {
            args.push("--no-edit");
        }
        args.push("--end-of-options");
        args.extend(sources);

        self.run_git(&args).map(drop).map_err(|err| match err {
            GitError::Command(mut failed)
                if failed.stdout.contains(OCTOPUS_FAILED)
                    || failed.stderr.contains(OCTOPUS_FAILED) =>
            {
                OCTOPUS_SUMMARY.clone_into(&mut failed.summary);
                GitError::Command(failed)
            }
            other => other,
        })
    }
}
