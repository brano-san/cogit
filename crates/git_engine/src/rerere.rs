//! `git rerere`: recorded conflict resolutions, and `git range-diff` for a rewritten branch.

use crate::{GitOutput, RepoHandle, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RerereStatus {
    pub enabled: bool,
    /// Conflicted paths rerere resolved from a recorded resolution.
    pub resolved: Vec<String>,
    /// Conflicted paths it has no resolution for yet.
    pub remaining: Vec<String>,
}

fn lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

impl RepoHandle {
    #[must_use]
    pub fn rerere_enabled(&self) -> bool {
        self.repo
            .config_snapshot()
            .boolean("rerere.enabled")
            .unwrap_or_else(|| self.repo.common_dir().join("rr-cache").is_dir())
    }

    pub fn rerere_status(&self) -> Result<RerereStatus> {
        if !self.rerere_enabled() {
            return Ok(RerereStatus::default());
        }
        let tracked = lines(&self.read_git(&["rerere", "status"])?);
        let remaining = lines(&self.read_git(&["rerere", "remaining"])?);
        let resolved = tracked
            .into_iter()
            .filter(|path| !remaining.contains(path))
            .collect();
        Ok(RerereStatus {
            enabled: true,
            resolved,
            remaining,
        })
    }

    /// Drops the recorded resolution and puts the conflict markers back.
    pub fn rerere_forget(&self, paths: &[String]) -> Result<()> {
        let mut args = vec!["rerere", "forget", "--"];
        args.extend(paths.iter().map(String::as_str));
        self.run_git_literal(&args).map(drop)
    }

    /// `before...after`: the commits of each side paired up and their patches compared.
    pub fn range_diff(&self, before: &str, after: &str) -> Result<GitOutput> {
        let range = format!("{before}...{after}");
        self.run_git(&["range-diff", "--no-color", &range])
    }
}
