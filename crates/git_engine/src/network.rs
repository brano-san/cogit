use crate::{GitCommandError, GitError, GitOutput, RepoHandle, Result};
use std::io::Read as _;
use std::process::Stdio;

impl RepoHandle {
    /// Remote names, read from the config rather than by spawning `git remote`.
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

    pub fn fetch(&self, remote: &str, on_line: impl FnMut(&str)) -> Result<()> {
        self.run_streaming(&["fetch", "--progress", "--prune", remote], on_line)
    }

    /// `--ff-only`: a pull that cannot fast-forward is a merge, and a merge started behind
    /// the user's back is exactly the surprise a Git client must not produce.
    pub fn pull(&self, remote: &str, ff_only: bool, on_line: impl FnMut(&str)) -> Result<()> {
        let mut args = vec!["pull", "--progress", remote];
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
        on_line: impl FnMut(&str),
    ) -> Result<()> {
        let mut args = vec!["push", "--progress"];
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

    /// Network commands can run for minutes, so their output is delivered as it appears
    /// instead of after the process exits. Git writes progress to `stderr`, overwriting
    /// the line with `\r`, so both separators end a line here.
    fn stream_git(&self, args: &[&str], mut on_line: impl FnMut(&str)) -> Result<GitOutput> {
        let command = format!("git {}", args.join(" "));
        let started = std::time::Instant::now();
        tracing::info!(command = %command, "running git");

        let mut child = self
            .base_git(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // stdout is drained on its own thread: a full pipe on either stream deadlocks.
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
        let result = GitOutput {
            command,
            exit_code: status.code(),
            stdout: GitCommandError::cap_stream(stdout),
            stderr: GitCommandError::cap_stream(stderr_text),
            duration_ms,
        };
        self.journal_entry(result.clone());
        Ok(result)
    }
}
