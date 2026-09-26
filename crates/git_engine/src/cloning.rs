//! Repository ▸ Clone… (F-575): what the server has, asked without writing anything, then
//! the clone itself through the system git.

use crate::network::{NetworkStop, SILENCE, Streamed, auth_config};
use crate::pulse::{BATCH_SSH, QUIET, STALL_LIMITS};
use crate::{CommandSink, GitCommandError, GitError, GitOutput, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The branches a clone can check out, as `git ls-remote --symref` listed them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranches {
    /// What the server's HEAD names: the branch a clone checks out unless told otherwise.
    pub default_branch: Option<String>,
    pub branches: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CloneRequest {
    /// A URL, or the folder of a repository on this machine.
    pub source: String,
    /// A full path to a folder that is missing or empty.
    pub target: String,
    pub submodules: bool,
    /// Off: only the branch checked out is fetched (`--single-branch`).
    pub all_branches: bool,
    /// `None` checks out what the server's HEAD names.
    pub branch: Option<String>,
    /// A partial clone: files larger than this many megabytes stay on the server.
    pub skip_larger_than_mb: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CloneDestination {
    Missing,
    Empty,
    NotEmpty,
    File,
    Unreadable,
}

#[must_use]
pub fn clone_destination(path: &Path) -> CloneDestination {
    match std::fs::read_dir(path) {
        Ok(mut entries) => match entries.next() {
            None => CloneDestination::Empty,
            Some(_) => CloneDestination::NotEmpty,
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => CloneDestination::Missing,
        Err(_) if path.is_file() => CloneDestination::File,
        Err(err) => {
            tracing::error!(error = ?err, path = %path.display(), context = "reading a clone's destination");
            CloneDestination::Unreadable
        }
    }
}

/// Asked as the background check asks (R-353, R-354): nobody is prompted, a stored
/// credential still answers, and nothing is written anywhere.
pub fn remote_branches(
    source: &str,
    token: Option<&str>,
    journal: Option<&CommandSink>,
) -> Result<RemoteBranches> {
    let source = source.trim();
    let header = token.and_then(|token| auth_config(source, token));
    let mut args: Vec<&str> = STALL_LIMITS.to_vec();
    if !own_ssh_command() {
        args.extend(["-c", BATCH_SSH]);
    }
    if let Some(header) = &header {
        args.extend(["-c", header.as_str()]);
    }
    args.extend([
        "ls-remote",
        "--symref",
        "--",
        source,
        "HEAD",
        "refs/heads/*",
    ]);

    let command = crate::redact_command(&args);
    let started = std::time::Instant::now();
    tracing::info!(%command, "running git");
    let mut process = outside_any_repository();
    process.envs(QUIET.iter().copied()).args(&args);
    let output = crate::children::output(&mut process).map_err(crate::runner::not_started)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if output.status.success() {
        tracing::info!(%command, elapsed_ms = crate::runner::elapsed_ms(started), "git finished");
        return Ok(listing(&stdout));
    }
    let record = GitOutput::record(
        Path::new(&crate::runner::redact_url(source)),
        command,
        output.status.code(),
        &stdout,
        &String::from_utf8_lossy(&output.stderr),
        crate::runner::elapsed_ms(started),
    );
    if let Some(sink) = journal {
        sink(record.clone());
    }
    Err(GitError::Command(Box::new(GitCommandError::from_output(
        record,
    ))))
}

fn listing(text: &str) -> RemoteBranches {
    let mut found = RemoteBranches::default();
    for line in text.lines() {
        let Some((left, name)) = line.split_once('\t') else {
            continue;
        };
        if let Some(target) = left.strip_prefix("ref: ") {
            if name == "HEAD" {
                found.default_branch = target.strip_prefix("refs/heads/").map(str::to_owned);
            }
        } else if let Some(branch) = name.strip_prefix("refs/heads/") {
            found.branches.push(branch.to_owned());
        }
    }
    found
}

/// A process tree ended from outside cleans up nothing, so a cancelled clone's folder is
/// removed here as git removes it after its own failures: whole, or emptied if it was empty.
pub fn clone_repository(
    request: &CloneRequest,
    token: Option<&str>,
    stop: &NetworkStop,
    journal: Option<&CommandSink>,
    on_line: impl FnMut(&str),
) -> Result<PathBuf> {
    let source = request.source.trim();
    let target = PathBuf::from(request.target.trim());
    if source.is_empty() {
        return Err(GitError::InvalidState(
            "a clone needs a repository to clone".to_owned(),
        ));
    }
    if !target.is_absolute() {
        return Err(GitError::InvalidState(format!(
            "a clone goes to a full path, not {}",
            target.display()
        )));
    }
    let before = clone_destination(&target);
    let header = token.and_then(|token| auth_config(source, token));
    let args = clone_args(
        request,
        source,
        &target.to_string_lossy(),
        header.as_deref(),
    );
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut process = outside_any_repository();
    process.args(&args);

    let to = Streamed {
        root: &target,
        stop: Some(stop),
        journal,
    };
    let result = to.run(process, &args, on_line, SILENCE);
    if matches!(result, Err(GitError::Cancelled(_))) {
        remove_leftovers(&target, before);
    }
    result.map(|()| target)
}

fn clone_args(
    request: &CloneRequest,
    source: &str,
    target: &str,
    header: Option<&str>,
) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    if let Some(header) = header {
        args.extend(["-c".to_owned(), header.to_owned()]);
    }
    args.extend(["clone".to_owned(), "--progress".to_owned()]);
    if request.submodules {
        args.push("--recurse-submodules".to_owned());
    }
    if !request.all_branches {
        args.push("--single-branch".to_owned());
    }
    if let Some(branch) = request
        .branch
        .as_deref()
        .map(str::trim)
        .filter(|branch| !branch.is_empty())
    {
        args.extend(["--branch".to_owned(), branch.to_owned()]);
    }
    if let Some(megabytes) = request.skip_larger_than_mb {
        args.push(format!("--filter=blob:limit={megabytes}m"));
        if request.submodules {
            args.push("--also-filter-submodules".to_owned());
        }
    }
    args.extend(["--".to_owned(), source.to_owned(), target.to_owned()]);
    args
}

/// So that a repository around Cogit's own folder lends no config, and no `origin`.
fn outside_any_repository() -> Command {
    let mut process = crate::runner::git_command();
    process.current_dir(std::env::temp_dir());
    process
}

fn own_ssh_command() -> bool {
    std::env::var_os("GIT_SSH_COMMAND").is_some()
        || std::env::var_os("GIT_SSH").is_some()
        || crate::runner::bare_git(&["config", "--get", "core.sshCommand"])
            .is_ok_and(|out| out.exit_code == Some(0) && !out.stdout.trim().is_empty())
}

fn remove_leftovers(target: &Path, before: CloneDestination) {
    let removed = match before {
        CloneDestination::Missing if target.exists() => std::fs::remove_dir_all(target),
        CloneDestination::Empty => empty_folder(target),
        _ => return,
    };
    match removed {
        Ok(()) => tracing::info!(target = %target.display(), "removed what a cancelled clone left"),
        Err(err) => {
            tracing::error!(error = ?err, target = %target.display(), context = "removing what a cancelled clone left")
        }
    }
}

fn empty_folder(folder: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(folder)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            std::fs::remove_dir_all(entry.path())?;
        } else {
            std::fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

/// Hosts whose repositories live at `owner/name`: a bare link to one is a repository URL.
const FORGES: &[&str] = &["github.com", "gitlab.com", "bitbucket.org", "codeberg.org"];

/// A copied link to a repository, if that is all `text` holds; not any web address.
#[must_use]
pub fn repository_url_in(text: &str) -> Option<String> {
    let url = text.trim();
    if url.is_empty()
        || url.len() > 2048
        || url.starts_with('-')
        || url.chars().any(char::is_whitespace)
    {
        return None;
    }
    let lower = url.to_ascii_lowercase();
    let recognized = match lower.split_once("://") {
        Some(("ssh" | "git" | "git+ssh" | "ssh+git" | "file", rest)) => rest.len() > 1,
        Some(("http" | "https", rest)) => web_repository(rest),
        Some(_) => false,
        None => scp_like(&lower),
    };
    recognized.then(|| url.to_owned())
}

fn web_repository(rest: &str) -> bool {
    if rest.contains(['?', '#']) {
        return false;
    }
    let Some((authority, path)) = rest.split_once('/') else {
        return false;
    };
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = host.split_once(':').map_or(host, |(host, _)| host);
    let segments: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let Some(last) = segments.last() else {
        return false;
    };
    last.ends_with(".git")
        || segments.contains(&"_git")
        || (FORGES.contains(&host) && segments.len() == 2)
}

/// `git@host:owner/name.git`, the form SSH remotes are copied in. A drive letter is one
/// character and has no user, so `C:\work` is never taken for a host.
fn scp_like(text: &str) -> bool {
    let Some((left, path)) = text.split_once(':') else {
        return false;
    };
    let Some((user, host)) = left.split_once('@') else {
        return false;
    };
    !user.is_empty()
        && host.len() > 1
        && !left.contains(['/', '\\'])
        && !path.is_empty()
        && !path.starts_with('\\')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> CloneRequest {
        CloneRequest {
            source: "https://example.com/team/app.git".to_owned(),
            target: "D:/src/app".to_owned(),
            submodules: true,
            all_branches: true,
            branch: None,
            skip_larger_than_mb: None,
        }
    }

    fn args_of(request: &CloneRequest) -> Vec<String> {
        clone_args(request, &request.source, &request.target, None)
    }

    #[test]
    fn a_default_clone_asks_for_progress_and_submodules() {
        assert_eq!(
            args_of(&request()),
            [
                "clone",
                "--progress",
                "--recurse-submodules",
                "--",
                "https://example.com/team/app.git",
                "D:/src/app"
            ]
        );
    }

    #[test]
    fn one_branch_and_a_size_limit_reach_git_as_its_own_options() {
        let args = args_of(&CloneRequest {
            all_branches: false,
            branch: Some("release".to_owned()),
            skip_larger_than_mb: Some(5),
            ..request()
        });
        for wanted in [
            "--single-branch",
            "--filter=blob:limit=5m",
            "--also-filter-submodules",
        ] {
            assert!(args.iter().any(|arg| arg == wanted), "{wanted}: {args:?}");
        }
        let branch = args.iter().position(|arg| arg == "--branch").unwrap();
        assert_eq!(args[branch + 1], "release");
    }

    #[test]
    fn a_partial_clone_without_submodules_filters_only_itself() {
        let args = args_of(&CloneRequest {
            submodules: false,
            skip_larger_than_mb: Some(1),
            ..request()
        });
        assert!(!args.iter().any(|arg| arg == "--also-filter-submodules"));
        assert!(!args.iter().any(|arg| arg == "--recurse-submodules"));
    }

    #[test]
    fn the_listing_names_the_default_branch_and_every_head() {
        let found = listing(
            "ref: refs/heads/main\tHEAD\n\
             1111111111111111111111111111111111111111\tHEAD\n\
             1111111111111111111111111111111111111111\trefs/heads/feature/x\n\
             1111111111111111111111111111111111111111\trefs/heads/main\n\
             ref: refs/remotes/origin/main\trefs/remotes/origin/HEAD\n",
        );
        assert_eq!(found.default_branch.as_deref(), Some("main"));
        assert_eq!(found.branches, ["feature/x", "main"]);
    }
}
