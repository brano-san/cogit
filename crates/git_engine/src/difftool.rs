//! `git difftool` on one file, started and left running (F-944).

use crate::runner::{not_started, redact_command};
use crate::{DiffSpec, GitError, RepoHandle, Result};
use std::process::Stdio;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// A tool that cannot start (unknown name, program not found) fails within this (git's
/// shell scripts take 1-2 s on Windows); one still running past it is taken as open.
const EARLY_FAILURE: Duration = Duration::from_secs(3);

const OWN_TOOL: &str = "cogit";

const CONSOLE_TOOLS: &[&str] = &["vimdiff", "nvimdiff", "emerge"];

pub const NO_TOOL: &str = "No external diff tool is configured. Set a program in Preferences ▸ Diff & Merge ▸ Diff Tool, or name one for Git: git config --global diff.tool <tool> (meld, kdiff3, vscode, bc, winmerge…).";

/// `diff.tool`, else `merge.tool`, as `git difftool` falls back; the last value wins.
#[must_use]
pub fn diff_tool_from_listing(listing: &str) -> Option<String> {
    let (mut diff, mut merge) = (None, None);
    for entry in listing.split('\0') {
        let Some((key, value)) = entry.split_once('\n') else {
            continue;
        };
        let value = value.trim();
        match key.to_ascii_lowercase().as_str() {
            "diff.tool" => diff = Some(value.to_owned()).filter(|v| !v.is_empty()),
            "merge.tool" => merge = Some(value.to_owned()).filter(|v| !v.is_empty()),
            _ => {}
        }
    }
    diff.or(merge)
}

fn console_tool(tool: &str) -> bool {
    let tool = tool.to_ascii_lowercase();
    CONSOLE_TOOLS.iter().any(|name| tool.starts_with(name))
}

/// `custom` is a `difftool.<tool>.cmd` replacing `tool`. The tool is always named: left to
/// itself, git replaces one it does not know with vimdiff, which hangs unseen without a console.
#[must_use]
pub fn difftool_args(
    tool: &str,
    revisions: &[String],
    cached: bool,
    path: &str,
    old_path: Option<&str>,
    custom: Option<&str>,
) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    if let Some(cmd) = custom {
        args.extend([
            "-c".to_owned(),
            format!("difftool.{OWN_TOOL}.cmd={cmd}"),
            "-c".to_owned(),
            format!("difftool.{OWN_TOOL}.trustExitCode=false"),
        ]);
    }
    let tool = if custom.is_some() { OWN_TOOL } else { tool };
    args.extend([
        "difftool".to_owned(),
        "--no-prompt".to_owned(),
        format!("--tool={tool}"),
    ]);
    if cached {
        args.push("--cached".to_owned());
    }
    let renamed = old_path.filter(|old| *old != path);
    if renamed.is_some() {
        args.push("-M".to_owned());
    }
    args.extend(revisions.iter().cloned());
    args.push("--".to_owned());
    args.extend(renamed.map(str::to_owned));
    args.push(path.to_owned());
    args
}

impl RepoHandle {
    pub fn diff_tool_name(&self) -> Result<Option<String>> {
        let listing = self.read_git(&["config", "--list", "-z"])?;
        Ok(diff_tool_from_listing(&listing))
    }

    fn difftool_revisions(&self, spec: &DiffSpec) -> Result<(Vec<String>, bool)> {
        Ok(match spec {
            DiffSpec::WorkTreeVsIndex => (Vec::new(), false),
            DiffSpec::IndexVsHead => (Vec::new(), true),
            DiffSpec::CommitVsWorkTree { oid } => (vec![oid.clone()], false),
            DiffSpec::CommitVsCommit { a, b } => (vec![a.clone(), b.clone()], false),
            DiffSpec::CommitVsParent { oid } => {
                let parent = match self.first_parent(oid)? {
                    Some(parent) => parent,
                    None => gix::ObjectId::empty_tree(self.repo.object_hash()).to_string(),
                };
                (vec![parent, oid.clone()], false)
            }
        })
    }

    /// Returns once the tool is up; git removes its temporary copies when the tool closes.
    pub fn launch_difftool(
        &self,
        spec: &DiffSpec,
        path: &str,
        old_path: Option<&str>,
        custom: Option<&str>,
    ) -> Result<()> {
        let tool = match custom {
            Some(_) => OWN_TOOL.to_owned(),
            None => match self.diff_tool_name()? {
                None => return Err(GitError::InvalidState(NO_TOOL.to_owned())),
                Some(tool) if console_tool(&tool) => {
                    return Err(GitError::InvalidState(format!(
                        "diff.tool `{tool}` runs in a terminal, which Cogit does not have. Set a program in Preferences ▸ Diff & Merge ▸ Diff Tool, or a windowed tool in diff.tool."
                    )));
                }
                Some(tool) => tool,
            },
        };
        let (revisions, cached) = self.difftool_revisions(spec)?;
        let args = difftool_args(&tool, &revisions, cached, path, old_path, custom);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let command = redact_command(&args);
        let started = Instant::now();
        let mut process = self.git_process(true);
        process
            .env("GIT_LITERAL_PATHSPECS", "1")
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        // Not tracked: the tool belongs to the user and outlives Cogit's exit.
        let mut child = process.spawn().map_err(not_started)?;
        tracing::info!(%command, "external diff tool started");
        let stderr = child.stderr.take();
        let (send, receive) = mpsc::channel();
        let logged = command.clone();
        std::thread::Builder::new()
            .name("difftool-wait".into())
            .spawn(move || {
                let mut said = Vec::new();
                if let Some(mut pipe) = stderr {
                    use std::io::Read as _;
                    if let Err(err) = pipe.read_to_end(&mut said) {
                        tracing::warn!(error = ?err, "reading the diff tool's stderr failed");
                    }
                }
                let status = child.wait();
                match &status {
                    Ok(status) => tracing::info!(
                        command = %logged,
                        exit_code = ?status.code(),
                        stderr = %String::from_utf8_lossy(&said),
                        "external diff tool ended"
                    ),
                    Err(err) => tracing::warn!(error = ?err, "waiting for the diff tool failed"),
                }
                let _ = send.send((status, said));
            })
            .map_err(|err| GitError::Io(format!("cannot watch the diff tool: {err}")))?;
        match receive.recv_timeout(EARLY_FAILURE) {
            Ok((Ok(status), stderr)) if !status.success() => {
                let output = std::process::Output {
                    status,
                    stdout: Vec::new(),
                    stderr,
                };
                Err(self.failed(command, &output, started))
            }
            Ok((Err(err), _)) => Err(GitError::Io(format!("waiting for git difftool: {err}"))),
            Ok(_) | Err(_) => Ok(()),
        }
    }
}
