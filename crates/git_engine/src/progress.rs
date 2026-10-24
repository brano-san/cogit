use crate::{GitError, RepoHandle, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
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

fn looks_like_oid(token: &str) -> bool {
    token.len() >= 4 && token.chars().all(|c| c.is_ascii_hexdigit())
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
            let oid = parts.next().unwrap_or_default();
            // A verb we do not know is still a step if it names an object: dropping it
            // would under-report the work left, which is worse than showing it oddly.
            if !VERBS.contains(&action) && !looks_like_oid(oid) {
                return None;
            }
            if oid.is_empty() {
                return None;
            }
            Some(RebaseStep {
                action: action.to_owned(),
                oid: oid.to_owned(),
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

/// What the list of steps left offers; `reword` would wait for an editor nobody sees.
const EDITABLE: &[&str] = &["pick", "edit", "squash", "fixup", "drop"];

fn count(dir: &Path, name: &str) -> Option<u32> {
    line(dir, name)?.parse().ok()
}

impl RepoHandle {
    /// The steps a running interactive rebase has left, replaced by `steps`: the same
    /// commits, in a new order or with new verbs. A plan with steps of other kinds (exec,
    /// label, merge) is git's to edit, and a list that is not the one left is stale.
    pub fn edit_rebase_todo(&self, steps: &[RebaseStep]) -> Result<()> {
        let merge = self.git_dir().join("rebase-merge");
        let text = std::fs::read_to_string(merge.join("git-rebase-todo"))
            .map_err(|_| GitError::InvalidState("no interactive rebase is running".to_owned()))?;
        let lines: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect();
        if lines
            .iter()
            .any(|line| !VERBS.contains(&line.split(' ').next().unwrap_or_default()))
        {
            return Err(GitError::InvalidState(
                "this rebase has steps Cogit cannot edit: use git rebase --edit-todo".to_owned(),
            ));
        }
        if let Some(step) = steps
            .iter()
            .find(|step| !EDITABLE.contains(&step.action.as_str()))
        {
            return Err(GitError::InvalidState(format!(
                "{} is not a step of the list",
                step.action
            )));
        }
        let mut left: Vec<String> = parse_todo(&merge, "git-rebase-todo")
            .into_iter()
            .map(|step| step.oid)
            .collect();
        let mut asked: Vec<String> = steps.iter().map(|step| step.oid.clone()).collect();
        left.sort_unstable();
        asked.sort_unstable();
        let same = left.len() == asked.len()
            && left
                .iter()
                .zip(&asked)
                .all(|(a, b)| a.starts_with(b.as_str()) || b.starts_with(a.as_str()));
        if !same {
            return Err(GitError::InvalidState(
                "the rebase moved on since the list was read: look again and retry".to_owned(),
            ));
        }

        let body: String = steps
            .iter()
            .map(|step| {
                format!(
                    "{} {} {}
",
                    step.action, step.oid, step.summary
                )
            })
            .collect();
        let todo = self.git_dir().join("cogit-rebase-todo");
        std::fs::write(&todo, body)?;
        // As `run_rebase_body`: git runs the sequence editor with the todo path appended.
        let editor = format!(
            "cp {}",
            crate::interactive::shell_quote(&crate::slash_path(&todo))
        );
        let result = self.run_git_with_env(
            &["rebase", "--edit-todo"],
            &[("GIT_SEQUENCE_EDITOR", &editor)],
        );
        let _ = std::fs::remove_file(&todo);
        result.map(drop)
    }

    /// `None` when no rebase is running. Never an error: a half-read state must still draw.
    pub fn rebase_progress(&self) -> Result<Option<RebaseProgress>> {
        let merge = self.git_dir().join("rebase-merge");
        if merge.is_dir() {
            // Counted as the steps left are: an `exec` or a `break` is not a commit.
            let done = u32::try_from(parse_todo(&merge, "done").len()).unwrap_or(0);
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
