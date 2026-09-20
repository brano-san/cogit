use crate::{RepoHandle, Result};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RebaseStep {
    pub action: String,
    pub oid: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RebaseProgress {
    pub applying: Option<String>,
    pub onto: Option<String>,
    pub done: u32,
    pub total: u32,
    pub todo: Vec<RebaseStep>,
}

const VERBS: &[&str] = &[
    "pick", "p", "reword", "r", "edit", "e", "squash", "s", "fixup", "f", "drop", "d",
];

fn line(dir: &Path, name: &str) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(name)).ok()?;
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn subject(dir: &Path, name: &str) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(name)).ok()?;
    let first = text.lines().find(|l| !l.trim().is_empty())?;
    Some(first.trim().to_owned())
}

/// `.git/rebase-merge/` is undocumented, so an unparsable line is skipped, never fatal.
fn parse_todo(dir: &Path, name: &str) -> Vec<RebaseStep> {
    let Ok(text) = std::fs::read_to_string(dir.join(name)) else {
        return Vec::new();
    };

    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let mut parts = l.splitn(3, ' ');
            let action = parts.next()?;
            if !VERBS.contains(&action) {
                return None;
            }
            Some(RebaseStep {
                action: action.to_owned(),
                oid: parts.next().unwrap_or_default().to_owned(),
                // Git writes the subject behind a comment marker in some versions.
                summary: parts
                    .next()
                    .unwrap_or_default()
                    .trim_start_matches("# ")
                    .to_owned(),
            })
        })
        .collect()
}

fn count(dir: &Path, name: &str) -> Option<u32> {
    line(dir, name)?.parse().ok()
}

impl RepoHandle {
    /// `None` when no rebase is running. Never an error: a half-read state must still draw.
    pub fn rebase_progress(&self) -> Result<Option<RebaseProgress>> {
        let merge = self.git_dir().join("rebase-merge");
        if merge.is_dir() {
            let done = std::fs::read_to_string(merge.join("done"))
                .map(|text| u32::try_from(text.lines().filter(|l| !l.trim().is_empty()).count()))
                .ok()
                .and_then(std::result::Result::ok)
                .unwrap_or(0);
            let todo = parse_todo(&merge, "git-rebase-todo");

            return Ok(Some(RebaseProgress {
                applying: subject(&merge, "message"),
                onto: line(&merge, "onto"),
                done,
                total: done + u32::try_from(todo.len()).unwrap_or(0),
                todo,
            }));
        }

        // The `am` backend, used by a plain `git rebase` over patches, keeps counters only.
        let apply = self.git_dir().join("rebase-apply");
        if apply.is_dir() {
            let done = count(&apply, "next").unwrap_or(0);
            return Ok(Some(RebaseProgress {
                applying: subject(&apply, "final-commit"),
                onto: line(&apply, "onto"),
                done,
                total: count(&apply, "last").unwrap_or(done),
                todo: Vec::new(),
            }));
        }

        Ok(None)
    }
}
