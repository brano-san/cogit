//! The user's own commands (F-954): a program and its words, never a shell.

use crate::{GitError, GitOutput, NetworkStop, Result};
use std::path::Path;

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
