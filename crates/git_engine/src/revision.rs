//! Inline checks behind pickers: does this text name a commit, is this a branch name. They
//! run on every keystroke and a "no" is an answer, so unlike `run_git` they leave no
//! journal entry and no error.

use crate::{RepoHandle, Result};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CommitPreview {
    pub oid: String,
    pub short_oid: String,
    pub subject: String,
    /// Author date, Unix seconds.
    #[specta(type = specta_typescript::Number)]
    pub date: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RevisionCheck {
    pub commit: Option<CommitPreview>,
    pub problem: Option<String>,
}

impl RepoHandle {
    /// `git rev-parse --verify <rev>^{commit}`, then the commit's subject and date.
    pub fn check_revision(&self, rev: &str) -> Result<RevisionCheck> {
        let rev = rev.trim();
        let missing = |problem: String| RevisionCheck {
            commit: None,
            problem: Some(problem),
        };
        if rev.is_empty() || rev.starts_with('-') {
            return Ok(missing(format!("\"{rev}\" is not a revision")));
        }
        let peeled = format!("{rev}^{{commit}}");
        let Some(oid) = self.probe(&["rev-parse", "--verify", "--quiet", &peeled])? else {
            return Ok(missing(format!("No commit matches \"{rev}\"")));
        };
        let oid = oid.trim().to_owned();
        let shown = self
            .probe(&[
                "show",
                "-s",
                "--format=%h%x00%ad%x00%s",
                "--date=unix",
                &oid,
            ])?
            .unwrap_or_default();
        let mut parts = shown.trim_end().splitn(3, '\0');
        let short_oid = parts.next().unwrap_or_default().to_owned();
        let date = parts.next().and_then(|d| d.parse().ok()).unwrap_or(0);
        let subject = parts.next().unwrap_or_default().to_owned();
        Ok(RevisionCheck {
            commit: Some(CommitPreview {
                oid,
                short_oid,
                subject,
                date,
            }),
            problem: None,
        })
    }

    /// `git check-ref-format --branch`: `None` when valid, else git's own complaint.
    pub fn check_branch_name(&self, name: &str) -> Result<Option<String>> {
        if name.is_empty() {
            return Ok(Some("Enter a branch name".to_owned()));
        }
        if name.starts_with('-') {
            return Ok(Some(format!("'{name}' is not a valid branch name")));
        }
        let mut process = crate::runner::base_command(self.root(), true);
        process.args(["check-ref-format", "--branch", name]);
        let output = crate::children::output(&mut process).map_err(crate::runner::not_started)?;
        if output.status.success() {
            return Ok(None);
        }
        let said = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Ok(Some(if said.is_empty() {
            format!("'{name}' is not a valid branch name")
        } else {
            said.trim_start_matches("fatal: ").to_owned()
        }))
    }

    /// Stdout when git succeeds, `None` when it exits non-zero.
    fn probe(&self, args: &[&str]) -> Result<Option<String>> {
        let mut process = crate::runner::base_command(self.root(), true);
        process.args(args);
        let output = crate::children::output(&mut process).map_err(crate::runner::not_started)?;
        Ok(output
            .status
            .success()
            .then(|| String::from_utf8_lossy_owned(output.stdout)))
    }
}
