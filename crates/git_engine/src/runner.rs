use crate::{GitCommandError, GitError, RepoHandle, Result};
use serde::Serialize;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

pub type CommandSink = Arc<dyn Fn(GitOutput) + Send + Sync>;

/// Per process, not per repository: the window is opened by id and does not care which
/// repository the run came from. Wrapping after four billion commands is not a scenario.
static NEXT_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

/// Cleared, not overridden — `GIT_DIR` and friends override `current_dir`, and unset must
/// stay unset (doc/12-risks.md, R-22).
const INHERITED_GIT_VARS: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_INDEX_VERSION",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_PREFIX",
    "GIT_NAMESPACE",
    "GIT_CEILING_DIRECTORIES",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
];

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GitOutput {
    /// Numbered so a window, a toast and a history row can all name the same run.
    pub id: u32,
    /// The repository root the command ran in. A record outlives the handle that made it.
    pub repo: String,
    pub command: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u32,
    /// What to call this in a title. Read off `command`, so the two cannot disagree.
    pub operation: String,
    pub severity: crate::outcome::Severity,
    /// One line over the output, never instead of it.
    pub summary: String,
    /// Unix epoch milliseconds. Not `DateTime`: the tree has no date crate, and the
    /// frontend formats every other timestamp from a number already.
    #[specta(type = specta_typescript::Number)]
    pub started_at_ms: u64,
}

impl GitOutput {
    /// Normalises both streams and works out how to describe the result. Every record
    /// in the journal goes through here, git commands and hook runs alike.
    #[must_use]
    pub fn record(
        repo: &Path,
        command: String,
        exit_code: Option<i32>,
        stdout: &str,
        stderr: &str,
        duration_ms: u32,
    ) -> Self {
        let stdout = crate::output_text::normalise(stdout);
        let stderr = crate::output_text::normalise(stderr);
        // The log file keeps what the window cannot show, and it is written here rather
        // than at the five call sites so that no path can capture output and lose it.
        tracing::debug!(%command, exit_code = ?exit_code, %stdout, %stderr, "git output");
        let stdout = crate::output_text::trim(&stdout);
        let stderr = crate::output_text::trim(&stderr);
        Self {
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            repo: repo.display().to_string(),
            operation: crate::outcome::operation_label(&command),
            severity: crate::outcome::severity_of(exit_code, &stderr),
            summary: crate::outcome::summarise(&stderr, &stdout),
            started_at_ms: started_at_ms(),
            command,
            exit_code,
            stdout,
            stderr,
            duration_ms,
        }
    }
}

impl RepoHandle {
    pub fn run_git(&self, args: &[&str]) -> Result<GitOutput> {
        self.spawn(args, false)
    }

    /// `GIT_OPTIONAL_LOCKS=0` keeps a background read off `index.lock`.
    pub fn run_git_reading(&self, args: &[&str]) -> Result<GitOutput> {
        self.spawn(args, true)
    }

    pub(crate) fn base_git(&self, args: &[&str]) -> Command {
        let mut command = base_command(self.root(), false);
        command.args(args);
        command
    }

    /// The whole stdout, unaltered, for output that is parsed rather than shown: the
    /// journal's record trims long output and rewrites `\r` (R-280). Failures still
    /// reach the journal with both streams.
    pub(crate) fn read_git(&self, args: &[&str]) -> Result<String> {
        let command = redact_command(args);
        let started = std::time::Instant::now();
        let mut process = base_command(self.root(), true);
        process.args(args);
        let output = crate::children::output(&mut process)?;
        let duration_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        tracing::debug!(%command, bytes = output.stdout.len(), duration_ms, "git read");

        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        let result = GitOutput::record(
            self.root(),
            command,
            output.status.code(),
            &String::from_utf8_lossy(&output.stdout),
            &String::from_utf8_lossy(&output.stderr),
            duration_ms,
        );
        self.journal_entry(result.clone());
        tracing::error!(command = %result.command, exit_code = ?result.exit_code, "git read failed");
        Err(GitError::Command(Box::new(GitCommandError::from_output(
            result,
        ))))
    }

    pub(crate) fn journal_entry(&self, entry: GitOutput) {
        if let Some(sink) = self.journal() {
            sink(entry);
        }
    }

    /// For the handful of commands that need a variable `base_command` deliberately pins.
    pub(crate) fn run_git_with_env(
        &self,
        args: &[&str],
        env: &[(&str, &str)],
    ) -> Result<GitOutput> {
        self.spawn_with(args, false, env)
    }

    /// `args`, `--` and the paths. A list too long for a Windows command line goes through
    /// stdin instead, still as one command (R-191).
    pub(crate) fn run_git_paths(&self, args: &[&str], paths: &[String]) -> Result<GitOutput> {
        let mut all = args.to_vec();
        if fits_command_line(paths) {
            all.push("--");
            all.extend(paths.iter().map(String::as_str));
            return self.run_git(&all);
        }
        all.extend(["--pathspec-from-file=-", "--pathspec-file-nul"]);
        self.spawn_fed(&all, false, &[], Some(paths.join("\0").as_bytes()))
    }

    fn spawn(&self, args: &[&str], reading: bool) -> Result<GitOutput> {
        self.spawn_with(args, reading, &[])
    }

    fn spawn_with(&self, args: &[&str], reading: bool, env: &[(&str, &str)]) -> Result<GitOutput> {
        self.spawn_fed(args, reading, env, None)
    }

    fn spawn_fed(
        &self,
        args: &[&str],
        reading: bool,
        env: &[(&str, &str)],
        input: Option<&[u8]>,
    ) -> Result<GitOutput> {
        let command = redact_command(args);
        let started = std::time::Instant::now();

        tracing::info!(command = %command, "running git");
        let mut process = base_command(self.root(), reading);
        for (key, value) in env {
            process.env(key, value);
        }
        process.args(args);
        let output = match input {
            Some(bytes) => crate::children::output_fed(&mut process, bytes)?,
            None => crate::children::output(&mut process)?,
        };

        let duration_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        let result = GitOutput::record(
            self.root(),
            command,
            output.status.code(),
            &String::from_utf8_lossy(&output.stdout),
            &String::from_utf8_lossy(&output.stderr),
            duration_ms,
        );

        self.journal_entry(result.clone());

        if output.status.success() {
            return Ok(result);
        }

        tracing::error!(
            command = %result.command,
            exit_code = ?result.exit_code,
            duration_ms,
            summary = %result.summary,
            "git failed"
        );
        Err(GitError::Command(Box::new(GitCommandError::from_output(
            result,
        ))))
    }
}

/// Windows caps a command line at 32 767 UTF-16 units; bytes overcount them, and the rest
/// is left for the executable and the options.
const PATHS_BUDGET: usize = 24_000;

pub(crate) fn fits_command_line(paths: &[String]) -> bool {
    paths.iter().map(|path| path.len() + 3).sum::<usize>() <= PATHS_BUDGET
}

/// For the few commands without `--pathspec-from-file`, such as `clean`.
pub(crate) fn command_line_batches(paths: &[String]) -> Vec<&[String]> {
    let mut batches = Vec::new();
    let (mut start, mut length) = (0, 0);
    for (index, path) in paths.iter().enumerate() {
        if index > start && length + path.len() + 3 > PATHS_BUDGET {
            batches.push(&paths[start..index]);
            (start, length) = (index, 0);
        }
        length += path.len() + 3;
    }
    if start < paths.len() {
        batches.push(&paths[start..]);
    }
    batches
}

/// Wall-clock start, for the history list. A clock that jumps backwards only misorders
/// a list; it must never stop a command from running, so a failure reads as zero.
fn started_at_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Without it every `git` call flashes a console window and pays for it (R-24).
#[cfg(windows)]
pub(crate) const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn base_command(root: &Path, reading: bool) -> Command {
    let mut command = Command::new("git");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.current_dir(root);
    command.env("GIT_TERMINAL_PROMPT", "0");
    command.env("LC_ALL", "C");
    // `--continue` opens an editor, and with no terminal it hangs forever (R-26).
    command.env("GIT_EDITOR", "true");
    command.env("GIT_SEQUENCE_EDITOR", "true");
    if reading {
        command.env("GIT_OPTIONAL_LOCKS", "0");
    }
    for variable in INHERITED_GIT_VARS {
        command.env_remove(variable);
    }
    command
}

const HIDDEN: &str = "<redacted>";

/// This string reaches the journal, the log file and the error dialog (INV-05).
pub fn redact_command(args: &[&str]) -> String {
    let parts: Vec<String> = args.iter().map(|arg| redact_arg(arg)).collect();
    format!("git {}", parts.join(" "))
}

fn redact_arg(arg: &str) -> String {
    if let Some((key, _)) = arg.split_once('=')
        && key.eq_ignore_ascii_case("http.extraheader")
    {
        return format!("{key}={HIDDEN}");
    }
    redact_url(arg)
}

/// Only `scheme://user:secret@host` counts: a refspec and an SSH path also carry colons.
fn redact_url(arg: &str) -> String {
    let Some((scheme, rest)) = arg.split_once("://") else {
        return arg.to_owned();
    };
    let Some((authority, tail)) = rest.split_once('@') else {
        return arg.to_owned();
    };
    let Some((user, _)) = authority.split_once(':') else {
        return arg.to_owned();
    };
    format!("{scheme}://{user}:{HIDDEN}@{tail}")
}

/// Which `git` the writes actually go through. Run outside any repository, so it answers
/// before one is open and cannot fail on a broken working directory.
pub fn git_version() -> Result<String> {
    Ok(bare_git(&["--version"])?.stdout.trim().to_owned())
}

#[must_use]
pub fn gix_version() -> &'static str {
    let agent = gix::env::agent();
    agent.strip_prefix("oxide-").unwrap_or(agent)
}

/// What `git` said when it ran outside any repository.
pub(crate) struct BareOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// `git` with no repository behind it: the version, the user's own config. Output is
/// parsed, so `LC_ALL=C`. The streams are never logged — this reads config files, and
/// config files hold tokens in `url.*` and `http.*` (doc/12-risks.md, R-155).
pub(crate) fn bare_git(args: &[&str]) -> Result<BareOutput> {
    let mut command = Command::new("git");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.env("GIT_TERMINAL_PROMPT", "0");
    command.env("LC_ALL", "C");
    for variable in INHERITED_GIT_VARS {
        command.env_remove(variable);
    }
    let output = crate::children::output(command.args(args))?;
    tracing::debug!(command = %redact_command(args), exit_code = ?output.status.code(), "git without a repository");
    Ok(BareOutput {
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}
