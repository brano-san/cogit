use crate::children;
use crate::runner::command_for;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

/// What `git --version` said for a program chosen in Preferences.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GitProbe {
    pub valid: bool,
    pub version: Option<String>,
    pub error: Option<String>,
    /// `2.45` when the version works but is older than `MIN_GIT`: a warning, not a refusal.
    pub older_than: Option<String>,
}

impl GitProbe {
    fn invalid(error: String) -> Self {
        Self {
            valid: false,
            version: None,
            error: Some(error),
            older_than: None,
        }
    }
}

/// `2.41.0` from `git version 2.41.0.windows.1`: the numbers only.
#[must_use]
pub fn parse_git_version(stdout: &str) -> Option<String> {
    let word = stdout
        .trim()
        .strip_prefix("git version ")?
        .split_whitespace()
        .next()?;
    let numbers: Vec<&str> = word
        .split('.')
        .take_while(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
        .collect();
    (!numbers.is_empty()).then(|| numbers.join("."))
}

/// Preferences ▸ Git executable while Cogit runs: probed as the startup check does, and run
/// from the next command on only if it works. One that does not leaves the git in use and
/// says why, in the probe's own words.
///
/// # Errors
///
/// [`GitError::GitNotFound`] with the probe's reason when `program` is not a working git.
pub fn switch_git(program: &str, timeout: Duration) -> Result<GitProbe, crate::GitError> {
    let program = resolve_program(program);
    let probe = probe_program(&program, timeout, true);
    if !probe.valid {
        return Err(crate::GitError::GitNotFound(probe.error.unwrap_or_else(
            || format!("{} is not a working git", program.display()),
        )));
    }
    crate::runner::use_git_program(program);
    Ok(probe)
}

fn resolve_program(program: &str) -> std::path::PathBuf {
    match program.trim() {
        "" | "git" => crate::git_candidates::default_program(),
        trimmed => trimmed.into(),
    }
}

#[must_use]
pub fn probe_git(program: &str, timeout: Duration) -> GitProbe {
    probe_program(&resolve_program(program), timeout, true)
}

pub(crate) fn probe_program(path: &Path, timeout: Duration, remember_version: bool) -> GitProbe {
    let program = path.display();
    let mut command = command_for(path);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let (child, tracked) = match children::spawn(&mut command) {
        Ok(spawned) => spawned,
        Err(err) => return GitProbe::invalid(format!("cannot run {program}: {err}")),
    };
    let pid = child.id();
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _tracked = tracked;
        let _ = send.send(child.wait_with_output());
    });
    let output = match receive.recv_timeout(timeout) {
        Ok(Ok(output)) => output,
        Ok(Err(err)) => return GitProbe::invalid(format!("cannot run {program}: {err}")),
        Err(_) => {
            if let Err(err) = children::stop_tree(pid) {
                tracing::error!(error = ?err, context = "stopping a git probe that timed out");
            }
            return GitProbe::invalid(format!(
                "{program} timed out after {} s",
                timeout.as_secs().max(1)
            ));
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    if remember_version && output.status.success() {
        // `git_version` would start the same program to hear the same line.
        crate::runner::GIT_VERSION.remember(path.into(), stdout.trim().to_owned());
    }
    match parse_git_version(&stdout) {
        Some(version) if output.status.success() => GitProbe {
            valid: true,
            older_than: crate::git_candidates::is_below_min_git(&version).then(|| {
                format!(
                    "{}.{}",
                    crate::git_candidates::MIN_GIT.0,
                    crate::git_candidates::MIN_GIT.1
                )
            }),
            version: Some(version),
            error: None,
        },
        _ => {
            let said = String::from_utf8_lossy(&output.stderr);
            let said = if said.trim().is_empty() { stdout } else { said };
            GitProbe::invalid(format!("{program} is not git: {}", said.trim()))
        }
    }
}
