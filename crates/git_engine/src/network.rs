use crate::{GitCommandError, GitError, GitOutput, RepoHandle, Result};
use std::io::Read as _;
use std::process::Stdio;

impl RepoHandle {
    pub fn remotes(&self) -> Result<Vec<String>> {
        let mut names: Vec<String> = self
            .repo
            .remote_names()
            .into_iter()
            .map(|name| name.to_string())
            .collect();
        names.sort();
        Ok(names)
    }

    pub fn remote_url(&self, name: &str) -> Option<String> {
        self.repo
            .find_remote(name)
            .ok()?
            .url(gix::remote::Direction::Push)
            .map(|url| url.to_bstring().to_string())
    }

    pub fn fetch(
        &self,
        remote: &str,
        token: Option<&str>,
        on_line: impl FnMut(&str),
    ) -> Result<()> {
        let header = auth_arg(token);
        let mut args = prefix(&header);
        args.extend(["fetch", "--progress", "--prune", remote]);
        self.run_streaming(&args, on_line)
    }

    /// `--ff-only`: a pull that cannot fast-forward is a merge, and a merge started behind
    /// the user's back is exactly the surprise a Git client must not produce.
    pub fn pull(
        &self,
        remote: &str,
        ff_only: bool,
        token: Option<&str>,
        on_line: impl FnMut(&str),
    ) -> Result<()> {
        let header = auth_arg(token);
        let mut args = prefix(&header);
        args.extend(["pull", "--progress", remote]);
        if ff_only {
            args.push("--ff-only");
        }
        self.run_streaming(&args, on_line)
    }

    pub fn push(
        &self,
        remote: &str,
        refspec: Option<&str>,
        force: bool,
        token: Option<&str>,
        on_line: impl FnMut(&str),
    ) -> Result<()> {
        let header = auth_arg(token);
        let mut args = prefix(&header);
        args.push("push");
        args.push("--progress");
        if force {
            // Never a bare `--force`: it overwrites work that arrived after our last fetch.
            args.push("--force-with-lease");
        }
        args.push(remote);
        if let Some(refspec) = refspec {
            args.push(refspec);
        }
        self.run_streaming(&args, on_line)
    }

    fn run_streaming(&self, args: &[&str], on_line: impl FnMut(&str)) -> Result<()> {
        let out = self.stream_git(args, on_line)?;
        if out.exit_code == Some(0) {
            return Ok(());
        }
        Err(GitError::Command(GitCommandError {
            command: out.command,
            exit_code: out.exit_code,
            stdout: out.stdout,
            stderr: out.stderr,
        }))
    }

    /// Delivered as it appears, not after the process exits.
    fn stream_git(&self, args: &[&str], mut on_line: impl FnMut(&str)) -> Result<GitOutput> {
        let command = crate::redact_command(args);
        let started = std::time::Instant::now();
        tracing::info!(command = %command, "running git");

        let mut child = self
            .base_git(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let mut stdout_pipe = child.stdout.take();
        let stdout_reader = std::thread::spawn(move || {
            let mut buffer = Vec::new();
            if let Some(pipe) = stdout_pipe.as_mut() {
                let _ = pipe.read_to_end(&mut buffer);
            }
            buffer
        });

        let mut stderr_text = String::new();
        if let Some(pipe) = child.stderr.as_mut() {
            let mut chunk = [0_u8; 4096];
            let mut pending = String::new();
            while let Ok(read) = pipe.read(&mut chunk) {
                if read == 0 {
                    break;
                }
                let text = String::from_utf8_lossy(&chunk[..read]);
                stderr_text.push_str(&text);
                pending.push_str(&text);

                while let Some(at) = pending.find(['\r', '\n']) {
                    let line = pending[..at].trim_end().to_owned();
                    pending.drain(..=at);
                    if !line.is_empty() {
                        on_line(&line);
                    }
                }
            }
            if !pending.trim().is_empty() {
                on_line(pending.trim_end());
            }
        }

        let status = child.wait()?;
        let stdout = stdout_reader
            .join()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default();

        let duration_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        let result = GitOutput::record(command, status.code(), &stdout, &stderr_text, duration_ms);
        self.journal_entry(result.clone());
        Ok(result)
    }
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (position, index) in [n >> 18, (n >> 12) & 63, (n >> 6) & 63, n & 63]
            .iter()
            .enumerate()
        {
            if position > chunk.len() {
                out.push('=');
            } else {
                out.push(char::from(ALPHABET[*index as usize]));
            }
        }
    }
    out
}

/// `x-access-token` is the username GitHub documents for a personal access token, and
/// GitLab and Bitbucket ignore the username when the password is a token.
#[must_use]
pub fn auth_header(token: &str) -> String {
    format!(
        "Authorization: Basic {}",
        base64(format!("x-access-token:{token}").as_bytes())
    )
}

/// SSH carries its own credentials through the agent, so a token there is noise at best.
#[must_use]
pub fn wants_auth(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

fn auth_arg(token: Option<&str>) -> Option<String> {
    token.map(|value| format!("http.extraHeader={}", auth_header(value)))
}

fn prefix(header: &Option<String>) -> Vec<&str> {
    match header {
        Some(value) => vec!["-c", value.as_str()],
        None => Vec::new(),
    }
}
