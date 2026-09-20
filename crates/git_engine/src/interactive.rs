use crate::{GitError, RepoHandle, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum TodoAction {
    Pick,
    Reword,
    Edit,
    Squash,
    Fixup,
    Drop,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TodoEntry {
    pub oid: String,
    pub action: TodoAction,
    pub message: Option<String>,
}

impl TodoAction {
    /// `reword` renders as `pick`: the message arrives by `exec`, so no editor opens.
    fn verb(self) -> &'static str {
        match self {
            Self::Pick | Self::Reword => "pick",
            Self::Edit => "edit",
            Self::Squash => "squash",
            Self::Fixup => "fixup",
            Self::Drop => "drop",
        }
    }
}

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

#[must_use]
pub fn render_todo(plan: &[TodoEntry]) -> String {
    render(plan, false)
}

/// A `break` after every applied commit, so a check command can run between steps.
/// `drop` applies nothing and `edit` already stops, so neither gets one (Git >= 2.20).
#[must_use]
pub fn render_todo_paused(plan: &[TodoEntry]) -> String {
    render(plan, true)
}

fn render(plan: &[TodoEntry], paused: bool) -> String {
    let mut out = String::new();
    for entry in plan {
        out.push_str(entry.action.verb());
        out.push(' ');
        out.push_str(&entry.oid);
        out.push('\n');

        let rewrites = matches!(entry.action, TodoAction::Reword | TodoAction::Squash);
        if let Some(message) = entry.message.as_deref().filter(|_| rewrites) {
            out.push_str("exec git commit --amend --no-verify -m ");
            out.push_str(&shell_quote(message));
            out.push('\n');
        }

        let stops_already = matches!(entry.action, TodoAction::Edit | TodoAction::Drop);
        if paused && !stops_already {
            out.push_str("break\n");
        }
    }
    out
}

impl RepoHandle {
    /// The plan Git itself would offer: every commit after `base`, oldest first, picked.
    pub fn rebase_todo(&self, base: &str) -> Result<Vec<TodoEntry>> {
        let range = format!("{base}..HEAD");
        let listing = self.run_git_reading(&["log", "--reverse", "--format=%H%x1f%s", &range])?;

        Ok(listing
            .stdout
            .lines()
            .filter_map(|line| line.split_once('\u{1f}'))
            .map(|(oid, subject)| TodoEntry {
                oid: oid.to_owned(),
                action: TodoAction::Pick,
                message: Some(subject.to_owned()),
            })
            .collect())
    }

    /// Runs `git rebase -i` with the plan already written, so nothing prompts.
    pub fn interactive_rebase(&self, base: &str, plan: &[TodoEntry]) -> Result<()> {
        self.run_rebase(base, plan, false)
    }

    /// Stops after every commit so the user can run a check before going on.
    pub fn interactive_rebase_paused(&self, base: &str, plan: &[TodoEntry]) -> Result<()> {
        self.run_rebase(base, plan, true)
    }

    fn run_rebase(&self, base: &str, plan: &[TodoEntry], paused: bool) -> Result<()> {
        if plan.is_empty() {
            return Err(GitError::InvalidState("the plan is empty".to_owned()));
        }
        let status = self.status()?;
        if status.staged > 0 || status.unstaged > 0 {
            return Err(GitError::InvalidState(
                "commit or stash your changes first: a rebase needs a clean working tree"
                    .to_owned(),
            ));
        }

        let todo = self.git_dir().join("cogit-rebase-todo");
        let body = if paused {
            render_todo_paused(plan)
        } else {
            render_todo(plan)
        };
        std::fs::write(&todo, body)?;

        let head = self
            .run_git_reading(&["rev-parse", "HEAD"])?
            .stdout
            .trim()
            .to_owned();
        self.run_git(&["update-ref", "ORIG_HEAD", &head])?;

        // Git runs the sequence editor through a shell with the todo path appended, so
        // copying our file over it is all the "editing" that is needed.
        let editor = format!(
            "cp {}",
            shell_quote(&todo.to_string_lossy().replace('\\', "/"))
        );
        let result =
            self.run_git_with_env(&["rebase", "-i", base], &[("GIT_SEQUENCE_EDITOR", &editor)]);
        let _ = std::fs::remove_file(&todo);
        result.map(drop)
    }
}
