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

/// `git rebase -i --root`: the plan then starts at the very first commit.
const ROOT: &str = "--root";

/// `Name <email>`, refused when either part could break out of the brackets.
fn author_identity(name: &str, email: &str) -> Result<String> {
    let (name, email) = (name.trim(), email.trim());
    let unsafe_char = |c: char| matches!(c, '<' | '>' | '\n' | '\r');
    if name.is_empty()
        || email.is_empty()
        || name.contains(unsafe_char)
        || email.contains(unsafe_char)
    {
        return Err(GitError::InvalidState(
            "an author needs a name and an email without angle brackets".to_owned(),
        ));
    }
    Ok(format!("{name} <{email}>"))
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
    /// `--root` plans every commit, the first one included.
    pub fn rebase_todo(&self, base: &str) -> Result<Vec<TodoEntry>> {
        let range = if base == ROOT {
            "HEAD".to_owned()
        } else {
            format!("{base}..HEAD")
        };
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

    /// Replays everything after the commit's parent with the new author stamped on by an
    /// `exec`, the way `reword` gets its message without an editor.
    pub fn edit_author(&self, rev: &str, name: &str, email: &str) -> Result<()> {
        let identity = author_identity(name, email)?;
        let target = self.resolve_commit(rev)?;
        let has_parent = self
            .repo
            .find_commit(target)
            .map_err(|err| GitError::Internal(format!("cannot read {rev}: {err}")))?
            .parent_ids()
            .next()
            .is_some();
        let base = if has_parent {
            format!("{target}^")
        } else {
            ROOT.to_owned()
        };
        let plan = self.rebase_todo(&base)?;
        let wanted = target.to_string();
        if !plan.iter().any(|entry| entry.oid == wanted) {
            return Err(GitError::InvalidState(format!(
                "{wanted} is not on the checked-out branch"
            )));
        }

        let mut body = String::new();
        for entry in &plan {
            body.push_str(&format!("pick {}\n", entry.oid));
            if entry.oid == wanted {
                body.push_str("exec git commit --amend --no-edit --no-verify --author=");
                body.push_str(&shell_quote(&identity));
                body.push('\n');
            }
        }
        self.run_rebase_body(&base, &body)
    }

    fn run_rebase(&self, base: &str, plan: &[TodoEntry], paused: bool) -> Result<()> {
        if plan.is_empty() {
            return Err(GitError::InvalidState("the plan is empty".to_owned()));
        }
        let body = if paused {
            render_todo_paused(plan)
        } else {
            render_todo(plan)
        };
        self.run_rebase_body(base, &body)
    }

    fn run_rebase_body(&self, base: &str, body: &str) -> Result<()> {
        let status = self.status()?;
        if status.staged > 0 || status.unstaged > 0 {
            return Err(GitError::InvalidState(
                "commit or stash your changes first: a rebase needs a clean working tree"
                    .to_owned(),
            ));
        }

        let todo = self.git_dir().join("cogit-rebase-todo");
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
