use crate::{GitCommandError, GitError, GitOutput, RepoHandle, Result};
use std::io::Read as _;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// How long git may say nothing before a network command is stopped: a server that took
/// the connection and went quiet, ssh over a dropped VPN. Long enough for a sign-in in the
/// browser (Git Credential Manager) and for a pre-push hook between two lines of output.
const SILENCE: Duration = Duration::from_secs(300);

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

    /// `token` is asked for the URL git will contact and answers with its token, if any.
    pub fn fetch(
        &self,
        remote: &str,
        token: impl FnOnce(&str) -> Option<String>,
        on_line: impl FnMut(&str),
    ) -> Result<()> {
        let header = self.auth_arg(remote, token);
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
        token: impl FnOnce(&str) -> Option<String>,
        on_line: impl FnMut(&str),
    ) -> Result<()> {
        let header = self.auth_arg(remote, token);
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
        token: impl FnOnce(&str) -> Option<String>,
        on_line: impl FnMut(&str),
    ) -> Result<()> {
        let header = self.auth_arg(remote, token);
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

    fn auth_arg(&self, remote: &str, token: impl FnOnce(&str) -> Option<String>) -> Option<String> {
        let url = self.remote_url(remote)?;
        auth_config(&url, &token(&url)?)
    }

    fn run_streaming(&self, args: &[&str], on_line: impl FnMut(&str)) -> Result<()> {
        self.run_streaming_within(args, on_line, SILENCE)
    }

    fn run_streaming_within(
        &self,
        args: &[&str],
        on_line: impl FnMut(&str),
        silence: Duration,
    ) -> Result<()> {
        let out = self.stream_git(args, on_line, silence)?;
        if out.exit_code == Some(0) {
            return Ok(());
        }
        Err(GitError::Command(Box::new(GitCommandError::from_output(
            out,
        ))))
    }

    /// Delivered as it appears, not after the process exits.
    fn stream_git(
        &self,
        args: &[&str],
        mut on_line: impl FnMut(&str),
        silence: Duration,
    ) -> Result<GitOutput> {
        let command = crate::redact_command(args);
        let started = Instant::now();
        tracing::info!(command = %command, "running git");

        let (mut child, _tracked) = crate::children::spawn(
            self.base_git(args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped()),
        )?;

        let heard = Arc::new(AtomicU64::new(0));
        let (finished, watching) = std::sync::mpsc::channel::<()>();
        let watchdog = {
            let (heard, pid) = (Arc::clone(&heard), child.id());
            std::thread::spawn(move || watch(pid, started, &heard, silence, &watching))
        };
        let mut stdout_pipe = child.stdout.take();
        let stdout_reader = {
            let heard = Arc::clone(&heard);
            std::thread::spawn(move || {
                let mut buffer = Vec::new();
                if let Some(pipe) = stdout_pipe.as_mut() {
                    let mut chunk = [0_u8; 4096];
                    while let Ok(read @ 1..) = pipe.read(&mut chunk) {
                        mark(&heard, started);
                        buffer.extend_from_slice(&chunk[..read]);
                    }
                }
                buffer
            })
        };

        let mut stderr = Progress::default();
        if let Some(pipe) = child.stderr.as_mut() {
            let mut chunk = [0_u8; 4096];
            while let Ok(read @ 1..) = pipe.read(&mut chunk) {
                mark(&heard, started);
                stderr.feed(&chunk[..read], &mut on_line);
            }
        }
        let stderr_text = stderr.finish(&mut on_line);
        // Before `wait`: an unreaped child keeps its pid, so the watchdog cannot hit another.
        drop(finished);
        let stopped = watchdog.join().unwrap_or(false);

        let status = child.wait()?;
        let stdout = stdout_reader
            .join()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default();

        let duration_ms = crate::runner::elapsed_ms(started);
        let mut result = GitOutput::record(
            self.root(),
            command,
            status.code(),
            &stdout,
            &stderr_text,
            duration_ms,
        );
        if stopped {
            result.summary = format!(
                "Stopped after {} s with no output from git",
                silence.as_secs()
            );
            tracing::warn!(command = %result.command, silence_s = silence.as_secs(), "stopped a silent network command");
        }
        self.journal_entry(result.clone());
        Ok(result)
    }
}

fn mark(heard: &AtomicU64, started: Instant) {
    heard.store(
        u64::from(crate::runner::elapsed_ms(started)),
        Ordering::Relaxed,
    );
}

/// `true` when it stopped the process tree: nothing heard from it for `silence`.
fn watch(
    pid: u32,
    started: Instant,
    heard: &AtomicU64,
    silence: Duration,
    finished: &std::sync::mpsc::Receiver<()>,
) -> bool {
    loop {
        let last = Duration::from_millis(heard.load(Ordering::Relaxed));
        let quiet = started.elapsed().saturating_sub(last);
        if quiet >= silence {
            if let Err(err) = crate::children::stop_tree(pid) {
                tracing::error!(error = ?err, pid, context = "stopping a silent network command");
            }
            return true;
        }
        if !matches!(
            finished.recv_timeout(silence - quiet),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ) {
            return false;
        }
    }
}

/// Git's progress on stderr, split into lines as it arrives. Kept as bytes until a line
/// ends: a read can stop inside a multi-byte character (a hook's Cyrillic message), and
/// decoding each read on its own turned that character into two U+FFFD.
#[derive(Default)]
struct Progress {
    all: Vec<u8>,
    pending: Vec<u8>,
}

impl Progress {
    fn feed(&mut self, chunk: &[u8], on_line: &mut impl FnMut(&str)) {
        self.all.extend_from_slice(chunk);
        self.pending.extend_from_slice(chunk);
        while let Some(at) = self.pending.iter().position(|b| matches!(b, b'\r' | b'\n')) {
            let line = String::from_utf8_lossy(&self.pending[..at])
                .trim_end()
                .to_owned();
            self.pending.drain(..=at);
            if !line.is_empty() {
                on_line(&line);
            }
        }
    }

    /// The last unterminated line, and everything as one text.
    fn finish(self, on_line: &mut impl FnMut(&str)) -> String {
        let rest = String::from_utf8_lossy(&self.pending);
        if !rest.trim().is_empty() {
            on_line(rest.trim_end());
        }
        String::from_utf8_lossy(&self.all).into_owned()
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

/// The `-c` argument that hands `token` to git for the host of `url` only. Git passes
/// every `-c` on to the git processes it starts, a submodule's fetch among them, so a bare
/// `http.extraHeader` went to every host that fetch contacted.
#[must_use]
pub fn auth_config(url: &str, token: &str) -> Option<String> {
    if !wants_auth(url) {
        return None;
    }
    let (scheme, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    if host.is_empty() {
        return None;
    }
    Some(format!(
        "http.{scheme}://{host}/.extraHeader={}",
        auth_header(token)
    ))
}

fn prefix(header: &Option<String>) -> Vec<&str> {
    match header {
        Some(value) => vec!["-c", value.as_str()],
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    // The queue of writes waits for a network command, so one that never ends held every
    // commit and stage of the repository until Cogit exited (03 §3 п.6).
    #[test]
    fn a_fetch_from_a_server_that_never_answers_is_stopped() {
        let f = test_fixtures::linear(1).unwrap();
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = server.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let held: Vec<_> = server.incoming().take(4).collect();
            std::thread::sleep(Duration::from_secs(120));
            drop(held);
        });
        f.git(&[
            "remote",
            "add",
            "quiet",
            &format!("git://127.0.0.1:{port}/x.git"),
        ])
        .unwrap();
        let repo = RepoHandle::open(f.path()).unwrap();
        let (done, finished) = std::sync::mpsc::channel();
        let started = std::time::Instant::now();

        std::thread::spawn(move || {
            let result = repo.run_streaming_within(
                &["fetch", "--progress", "quiet"],
                |_| {},
                Duration::from_secs(2),
            );
            let _ = done.send(result);
        });
        let result = finished
            .recv_timeout(Duration::from_secs(40))
            .expect("the fetch was still waiting for the server after 40 s");

        let Err(GitError::Command(failure)) = result else {
            panic!("a stopped fetch is a failed one: {result:?}");
        };
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "{:?}",
            started.elapsed()
        );
        assert!(failure.summary.contains("no output"), "{failure:?}");
    }

    #[test]
    fn a_character_split_between_two_reads_arrives_whole() {
        let text = "remote: Сборка отклонена\n".as_bytes();
        let cut = text.iter().position(|b| *b >= 0x80).unwrap() + 1;
        let mut lines = Vec::new();
        let mut progress = Progress::default();

        progress.feed(&text[..cut], &mut |line| lines.push(line.to_owned()));
        progress.feed(&text[cut..], &mut |line| lines.push(line.to_owned()));
        let all = progress.finish(&mut |line| lines.push(line.to_owned()));

        assert_eq!(lines, ["remote: Сборка отклонена"]);
        assert_eq!(all, "remote: Сборка отклонена\n");
    }

    #[test]
    fn carriage_returns_split_progress_and_the_tail_is_delivered() {
        let mut lines = Vec::new();
        let mut progress = Progress::default();

        progress.feed(b"Counting 1%\rCounting 2%\rdone", &mut |line| {
            lines.push(line.to_owned())
        });
        let _ = progress.finish(&mut |line| lines.push(line.to_owned()));

        assert_eq!(lines, ["Counting 1%", "Counting 2%", "done"]);
    }
}
