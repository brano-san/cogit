use crate::{CommitRow, RepoHandle, Result};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReflogEntry {
    pub selector: String,
    pub oid: String,
    pub action: String,
    pub message: String,
    #[specta(type = specta_typescript::Number)]
    pub timestamp: i64,
}

impl RepoHandle {
    /// Newest first, like `git reflog`.
    pub fn reflog(&self, limit: usize) -> Result<Vec<ReflogEntry>> {
        if self.head().is_err() || matches!(self.head()?, crate::Head::Unborn { .. }) {
            return Ok(Vec::new());
        }

        let count = limit.to_string();
        let out = self.run_git_reading(&[
            "reflog",
            "--max-count",
            &count,
            "--format=%H%x00%gd%x00%gs%x00%at",
        ])?;

        let mut entries = Vec::new();
        for line in out.stdout.lines().filter(|l| !l.is_empty()) {
            let mut parts = line.split('\0');
            let (Some(oid), Some(selector), Some(subject), Some(timestamp)) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                tracing::warn!(line, "skipping an unreadable reflog entry");
                continue;
            };
            let (action, message) = subject.split_once(": ").unwrap_or((subject, ""));
            entries.push(ReflogEntry {
                selector: selector.to_owned(),
                oid: oid.to_owned(),
                action: action.trim().to_owned(),
                message: message.trim().to_owned(),
                timestamp: timestamp.trim().parse().unwrap_or_default(),
            });
        }
        Ok(entries)
    }

    /// Commits the reflog remembers but no ref can reach — what a reset or a rebase left
    /// behind. This is the only way back to them from inside the client.
    pub fn lost_commits(&self, limit: usize) -> Result<Vec<CommitRow>> {
        let reachable = self.reachable_oids()?;
        let mut seen = HashSet::new();
        let mut lost = Vec::new();

        for entry in self.reflog(limit)? {
            if reachable.contains(&entry.oid) || !seen.insert(entry.oid.clone()) {
                continue;
            }
            if let Ok(details) = self.commit_details(&entry.oid) {
                lost.push(CommitRow {
                    oid: details.oid,
                    parents: details.parents,
                    summary: details.summary,
                    author_name: details.author.name,
                    author_email: details.author.email,
                    timestamp: details.author.timestamp,
                    tz_offset_minutes: details.author.tz_offset_minutes,
                });
            }
        }
        Ok(lost)
    }

    fn reachable_oids(&self) -> Result<HashSet<String>> {
        let mut reachable = HashSet::new();
        self.search_commits(&crate::CommitQuery::default(), 500, |chunk| {
            for row in chunk {
                reachable.insert(row.oid);
            }
            true
        })?;
        Ok(reachable)
    }
}
