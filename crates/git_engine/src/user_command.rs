//! The user's own commands (F-954): a program and its words, never a shell.

use crate::{GitError, GitOutput, NetworkStop, Result};
use std::path::Path;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ToolMetadata {
    pub branch: Option<String>,
    pub upstream: String,
    pub remote: String,
    pub issue: String,
}

impl crate::RepoHandle {
    pub fn tool_metadata(&self, global_pattern: &str, issue: bool) -> Result<ToolMetadata> {
        let (crate::Head::Branch { name, .. } | crate::Head::Unborn { name }) = self.head()? else {
            return Ok(ToolMetadata::default());
        };
        let full = gix::refs::FullName::try_from(format!("refs/heads/{name}"))
            .map_err(|err| GitError::InvalidState(err.to_string()))?;
        let remote = self
            .repo
            .branch_remote_name(full.shorten(), gix::remote::Direction::Fetch)
            .map(|remote| remote.as_bstr().to_string())
            .unwrap_or_default();
        let upstream = self
            .repo
            .branch_remote_tracking_ref_name(full.as_ref(), gix::remote::Direction::Fetch)
            .and_then(|tracking| {
                tracking.inspect_err(|err| {
                tracing::warn!(error = ?err, "cannot resolve the upstream for a user command");
            }).ok()
            })
            .map(|reference| reference.shorten().to_string())
            .unwrap_or_default();
        let issue = if issue {
            let config = self.repo.config_snapshot();
            let pattern = config.string("cogit.commitPrefixPattern");
            let pattern = pattern.as_ref().map(ToString::to_string);
            let trackers = self.issue_trackers();
            issue_from_branch(
                &name,
                pattern.as_deref().unwrap_or(global_pattern),
                &trackers,
            )
            .unwrap_or_default()
        } else {
            String::new()
        };
        Ok(ToolMetadata {
            branch: Some(name),
            upstream,
            remote,
            issue,
        })
    }
}

fn issue_from_branch(
    branch: &str,
    pattern: &str,
    trackers: &[crate::IssueTracker],
) -> Option<String> {
    pattern_match(pattern, branch, None).or_else(|| {
        trackers.iter().find_map(|tracker| {
            let regex = tracker.log_regex.as_deref()?;
            let mut lines = regex.lines().map(str::trim).filter(|line| !line.is_empty());
            pattern_match(lines.next()?, branch, lines.next())
        })
    })
}

fn pattern_match(pattern: &str, text: &str, inside: Option<&str>) -> Option<String> {
    if pattern.trim().is_empty() {
        return None;
    }
    let regex = regex::Regex::new(pattern)
        .inspect_err(|err| {
            tracing::warn!(error = ?err, "cannot use an issue pattern for a user command");
        })
        .ok()?;
    let captures = regex.captures(text)?;
    if let Some(inside) = inside {
        return pattern_match(inside, captures.get(0)?.as_str(), None);
    }
    let matched = if captures.len() == 1 {
        captures.get(0)
    } else {
        captures
            .iter()
            .skip(1)
            .flatten()
            .find(|part| !part.is_empty())
    };
    matched
        .filter(|part| !part.is_empty())
        .map(|part| part.as_str().to_owned())
}

#[derive(Debug, Clone)]
pub struct UserCommand<'a> {
    pub name: &'a str,
    pub program: &'a str,
    pub args: &'a [String],
    pub cwd: &'a Path,
}

impl UserCommand<'_> {
    #[must_use]
    pub fn line(&self) -> String {
        let words: Vec<&str> = std::iter::once(self.program)
            .chain(self.args.iter().map(String::as_str))
            .collect();
        crate::runner::redact_words(&words)
    }

    fn command(&self) -> Result<std::process::Command> {
        if !self.cwd.is_dir() {
            return Err(GitError::InvalidState(format!(
                "the working directory {} of {} does not exist",
                self.cwd.display(),
                self.name
            )));
        }
        let mut command = std::process::Command::new(self.program);
        command.args(self.args).current_dir(self.cwd);
        crate::runner::clear_inherited_git_vars(&mut command);
        Ok(command)
    }

    fn not_started(&self, err: &std::io::Error) -> GitError {
        if err.kind() == std::io::ErrorKind::NotFound {
            return GitError::InvalidState(format!(
                "{} was not found. Give its full path in Preferences ▸ Tools, or put its folder on PATH; a .cmd or .bat needs its full path.",
                self.program
            ));
        }
        GitError::Io(format!("cannot start {}: {err}", self.program))
    }

    pub fn run(&self, root: &Path, stop: &NetworkStop) -> Result<GitOutput> {
        let mut command = self.command()?;
        command.env("GIT_TERMINAL_PROMPT", "0");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            command.creation_flags(crate::runner::CREATE_NO_WINDOW);
        }
        let started = std::time::Instant::now();
        let finished = crate::hook_run::run_stoppable(&mut command, Some(stop))
            .map_err(|err| self.not_started(&err))?;
        let mut record = GitOutput::record(
            root,
            self.line(),
            finished.exit_code,
            &finished.stdout,
            &finished.stderr,
            crate::runner::elapsed_ms(started),
        );
        record.operation = format!("Tool: {}", self.name);
        if finished.exit_code.is_none() && stop.is_stopped() {
            record.summary = "Canceled by the user".to_owned();
            record.severity = crate::Severity::Warning;
        }
        tracing::info!(tool = self.name, exit = ?record.exit_code, duration_ms = record.duration_ms, "user command");
        Ok(record)
    }

    /// Not one of `children`: a GUI tool outlives Cogit. A thread reaps it.
    pub fn start(&self) -> Result<()> {
        use std::process::Stdio;
        let mut command = self.command()?;
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = command.spawn().map_err(|err| self.not_started(&err))?;
        tracing::info!(tool = self.name, pid = child.id(), "user command started");
        std::thread::spawn(move || {
            if let Err(err) = child.wait() {
                tracing::warn!(error = ?err, "cannot collect a user command");
            }
        });
        Ok(())
    }
}
