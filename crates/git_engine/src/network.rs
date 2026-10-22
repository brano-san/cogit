use crate::{GitError, GitOutput, RepoHandle, Result};
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// How long git may say nothing before a network command is stopped: a server that took
/// the connection and went quiet, ssh over a dropped VPN. Long enough for a sign-in in the
/// browser (Git Credential Manager) and for a pre-push hook between two lines of output.
pub(crate) const SILENCE: Duration = Duration::from_secs(300);

/// Asks a running fetch, pull or push to stop: its git process tree is ended and the
/// command fails with `GitError::Cancelled` (03 §3 п.6).
#[derive(Debug, Clone, Default)]
pub struct NetworkStop(Arc<Mutex<Stop>>);

#[derive(Debug, Default)]
struct Stop {
    asked: bool,
    pid: Option<u32>,
}

impl NetworkStop {
    /// `false` when it had been asked already.
    pub fn stop(&self) -> bool {
        let mut stop = self.lock();
        if stop.asked {
            return false;
        }
        stop.asked = true;
        // Under the lock: the command reaps its child only after `release`, so this pid
        // cannot have gone to another process yet.
        if let Some(pid) = stop.pid {
            end(pid);
        }
        true
    }

    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.lock().asked
    }

    pub(crate) fn hold(&self, pid: u32) {
        let mut stop = self.lock();
        stop.pid = Some(pid);
        if stop.asked {
            end(pid);
        }
    }

    /// Ends the process tree if it is still unreaped, so its pid is still its own.
    fn end_held(&self) -> bool {
        let stop = self.lock();
        let Some(pid) = stop.pid else {
            return false;
        };
        if let Err(err) = crate::children::stop_tree(pid) {
            tracing::error!(error = ?err, pid, context = "stopping a silent network command");
        }
        true
    }

    /// `try_wait` and forgetting the pid in one step under the lock: `stop` and the
    /// watchdog end `pid` only while it is held, so none can hit a process that got it
    /// after the reaping.
    pub(crate) fn reap(
        &self,
        child: &mut std::process::Child,
    ) -> std::io::Result<Option<ExitStatus>> {
        let mut stop = self.lock();
        let status = child.try_wait()?;
        if status.is_some() {
            stop.pid = None;
        }
        Ok(status)
    }

    pub(crate) fn release(&self) {
        self.lock().pid = None;
    }

    fn lock(&self) -> MutexGuard<'_, Stop> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn end(pid: u32) {
    match crate::children::stop_tree(pid) {
        Ok(()) => tracing::info!(pid, "stopped a network command on request"),
        Err(err) => {
            tracing::error!(error = ?err, pid, context = "stopping a cancelled network command")
        }
    }
}

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

    /// Where a push goes: `pushurl` if set, `insteadOf` and `pushInsteadOf` applied.
    pub fn remote_url(&self, name: &str) -> Option<String> {
        self.url_of(name, gix::remote::Direction::Push)
    }

    pub(crate) fn url_of(&self, name: &str, direction: gix::remote::Direction) -> Option<String> {
        self.repo
            .find_remote(name)
            .ok()?
            .url(direction)
            .map(|url| url.to_bstring().to_string())
    }

    /// `credential` is asked for the URL git will contact and answers with what it is signed
    /// in with, if anything.
    pub fn fetch(
        &self,
        remote: &str,
        credential: impl FnOnce(&str) -> Option<crate::Credential>,
        on_line: impl FnMut(&str),
    ) -> Result<()> {
        let auth = self.auth_env(remote, gix::remote::Direction::Fetch, credential);
        self.run_streaming(&["fetch", "--progress", "--prune", remote], &auth, on_line)
    }

    /// Every remote, as `git fetch --all` takes them (`remote.<name>.skipFetchAll` skips
    /// one), each with the credential of its own host.
    pub fn fetch_all(
        &self,
        credential: impl Fn(&str) -> Option<crate::Credential>,
        on_line: impl FnMut(&str),
    ) -> Result<()> {
        let mut pairs: Vec<(String, String)> = Vec::new();
        for remote in self.remotes()? {
            let pair = self.auth_pair(&remote, gix::remote::Direction::Fetch, &credential);
            // One header per host: two remotes on one host would send it twice.
            if let Some(pair) = pair.filter(|(key, _)| pairs.iter().all(|(k, _)| k != key)) {
                pairs.push(pair);
            }
        }
        let auth = config_env(&pairs);
        self.run_streaming(&["fetch", "--progress", "--prune", "--all"], &auth, on_line)
    }

    /// Remote ▸ Fetch More: every branch and tag of `remote`, whatever its
    /// refspec leaves out — a single-branch clone fetches one (R-553). `false`: nothing new came.
    pub fn fetch_more(
        &self,
        remote: &str,
        credential: impl FnOnce(&str) -> Option<crate::Credential>,
        on_line: impl FnMut(&str),
    ) -> Result<bool> {
        let tracking = format!("refs/remotes/{remote}/");
        let before = self.ref_tips(&[&tracking, "refs/tags/"]);
        let auth = self.auth_env(remote, gix::remote::Direction::Fetch, credential);
        let heads = format!("+refs/heads/*:{tracking}*");
        let args = ["fetch", "--progress", "--tags", remote, &heads];
        self.run_streaming(&args, &auth, on_line)?;
        Ok(self.ref_tips(&[&tracking, "refs/tags/"]) != before)
    }

    /// Remote ▸ Set Depth: a shallow clone's branches of `remote`, `depth` commits deep —
    /// deeper or shallower; 0 fetches all of it (`--unshallow`). A full clone is refused:
    /// `--depth` would make it shallow.
    pub fn fetch_depth(
        &self,
        remote: &str,
        depth: u32,
        credential: impl FnOnce(&str) -> Option<crate::Credential>,
        on_line: impl FnMut(&str),
    ) -> Result<()> {
        if !self.repo.is_shallow() {
            return Err(GitError::InvalidState(
                "The repository is not shallow: it has all of its history already.".to_owned(),
            ));
        }
        let auth = self.auth_env(remote, gix::remote::Direction::Fetch, credential);
        let depth = if depth == 0 {
            "--unshallow".to_owned()
        } else {
            format!("--depth={depth}")
        };
        self.run_streaming(&["fetch", "--progress", &depth, remote], &auth, on_line)
    }

    /// Names and targets of the refs under `prefixes`, sorted; unreadable ones are left out.
    pub(crate) fn ref_tips(&self, prefixes: &[&str]) -> Vec<(String, String)> {
        let Ok(platform) = self.repo.references() else {
            return Vec::new();
        };
        let mut tips = Vec::new();
        for prefix in prefixes {
            let Ok(refs) = platform.prefixed(*prefix) else {
                continue;
            };
            for reference in refs.flatten() {
                let target = reference
                    .try_id()
                    .map_or_else(String::new, |id| id.to_string());
                tips.push((reference.name().as_bstr().to_string(), target));
            }
        }
        tips.sort();
        tips
    }

    /// A branch whose tip is `oid`, for a detached HEAD: a local one first, else a
    /// remote-tracking one (`origin/main`); `None` when no branch ends there.
    pub fn branch_at(&self, oid: &str) -> Option<String> {
        let tips = self.ref_tips(&["refs/heads/", "refs/remotes/"]);
        let at = |prefix: &str| {
            tips.iter()
                .filter(|(name, target)| target == oid && !name.ends_with("/HEAD"))
                .find_map(|(name, _)| name.strip_prefix(prefix).map(str::to_owned))
        };
        at("refs/heads/").or_else(|| at("refs/remotes/"))
    }

    pub fn push(
        &self,
        remote: &str,
        refspec: Option<&str>,
        force: bool,
        credential: impl FnOnce(&str) -> Option<crate::Credential>,
        on_line: impl FnMut(&str),
    ) -> Result<()> {
        let first = refspec.is_none() && self.head_tracks_nothing();
        let refspec = refspec.or(first.then_some("HEAD"));
        self.push_with(remote, refspec, force, first, credential, on_line)
    }

    /// One refspec. `track`: the local branch it sends tracks what it becomes on the
    /// remote (`--set-upstream`), as the first push of a branch does (R-414, R-550).
    pub fn push_refspec(
        &self,
        remote: &str,
        refspec: &str,
        track: bool,
        credential: impl FnOnce(&str) -> Option<crate::Credential>,
        on_line: impl FnMut(&str),
    ) -> Result<()> {
        self.push_with(remote, Some(refspec), false, track, credential, on_line)
    }

    fn push_with(
        &self,
        remote: &str,
        refspec: Option<&str>,
        force: bool,
        track: bool,
        credential: impl FnOnce(&str) -> Option<crate::Credential>,
        on_line: impl FnMut(&str),
    ) -> Result<()> {
        let auth = self.auth_env(remote, gix::remote::Direction::Push, credential);
        let mut args = vec!["push"];
        args.push("--progress");
        if track {
            args.push("--set-upstream");
        }
        if force {
            // Never a bare `--force`: it overwrites work that arrived after our last fetch.
            args.push("--force-with-lease");
            // A lease alone passes when a background fetch moved the tracking ref.
            args.push("--force-if-includes");
        }
        args.push(remote);
        args.extend(refspec);
        self.run_streaming(&args, &auth, on_line)
    }

    /// HEAD is on a branch without an upstream as git decides it, which wants both
    /// `branch.<name>.remote` and `branch.<name>.merge`: a bare `git push` refuses such a
    /// branch (R-414), a bare `git pull` has nothing to merge into it (R-552).
    #[must_use]
    pub fn head_tracks_nothing(&self) -> bool {
        let Ok(Some(name)) = self.repo.head_name() else {
            return false;
        };
        let remote = self
            .repo
            .branch_remote_name(name.shorten(), gix::remote::Direction::Fetch);
        let merge = self
            .repo
            .branch_remote_ref_name(name.as_ref(), gix::remote::Direction::Fetch);
        remote.is_none() || merge.is_none()
    }

    /// The credential of the URL this direction contacts, not of the push URL for all (R-410),
    /// as the environment git reads it from: an argument sits in the process list.
    pub(crate) fn auth_env(
        &self,
        remote: &str,
        direction: gix::remote::Direction,
        credential: impl FnOnce(&str) -> Option<crate::Credential>,
    ) -> Vec<(String, String)> {
        config_env(
            &self
                .auth_pair(remote, direction, credential)
                .into_iter()
                .collect::<Vec<_>>(),
        )
    }

    fn auth_pair(
        &self,
        remote: &str,
        direction: gix::remote::Direction,
        credential: impl FnOnce(&str) -> Option<crate::Credential>,
    ) -> Option<(String, String)> {
        let url = self.url_of(remote, direction)?;
        crate::credentials::credential_config(&url, &credential(&url)?)
    }

    pub(crate) fn run_streaming(
        &self,
        args: &[&str],
        env: &[(String, String)],
        on_line: impl FnMut(&str),
    ) -> Result<()> {
        self.run_streaming_within(args, env, on_line, SILENCE)
    }

    /// `run_streaming` that hands back the record of a run that succeeded.
    pub(crate) fn run_streaming_recorded(
        &self,
        args: &[&str],
        env: &[(String, String)],
        on_line: impl FnMut(&str),
    ) -> Result<GitOutput> {
        let to = Streamed {
            root: self.root(),
            stop: self.stop.as_ref(),
            journal: &|entry| self.journal_entry(entry),
        };
        let (args, process) = self.network_command(args, env);
        to.run_recorded(process, &args, on_line, SILENCE)
    }

    /// A command that talks to a remote and shows no progress: stoppable and watched.
    pub(crate) fn run_network(&self, args: &[&str]) -> Result<()> {
        self.run_streaming(args, &[], |_| {})
    }

    /// `run_streaming` for output that is parsed: the whole stdout, a journal entry only
    /// when the command fails.
    pub(crate) fn run_streaming_read(
        &self,
        args: &[&str],
        env: &[(String, String)],
        on_line: impl FnMut(&str),
    ) -> Result<String> {
        let to = Streamed {
            root: self.root(),
            stop: self.stop.as_ref(),
            journal: &|entry| self.journal_entry(entry),
        };
        let (args, process) = self.network_command(args, env);
        to.run_read(process, &args, on_line, SILENCE)
    }

    fn run_streaming_within(
        &self,
        args: &[&str],
        env: &[(String, String)],
        on_line: impl FnMut(&str),
        silence: Duration,
    ) -> Result<()> {
        let to = Streamed {
            root: self.root(),
            stop: self.stop.as_ref(),
            journal: &|entry| self.journal_entry(entry),
        };
        let (args, process) = self.network_command(args, env);
        to.run(process, &args, on_line, silence)
    }

    /// The command with what keeps ssh from asking where nobody can answer (GR-06): a batch
    /// ssh of Cogit's own, or a refusing askpass for the user's. No HTTPS askpass either:
    /// one inherited from the terminal Cogit started in (VS Code's) asked there, and the
    /// command waited for it; a refusal comes back as `authRequired` instead (F-082). The arguments are returned
    /// because they are what the journal shows.
    fn network_command<'a>(
        &self,
        args: &[&'a str],
        env: &[(String, String)],
    ) -> (Vec<&'a str>, std::process::Command) {
        let own_ssh = self.has_own_ssh_command();
        let mut all: Vec<&str> = Vec::new();
        if !own_ssh {
            all.extend(["-c", crate::pulse::BATCH_SSH]);
        }
        all.extend_from_slice(args);
        let mut process = self.base_git(&all);
        process.envs(crate::pulse::askpass_refusal(own_ssh));
        process.env("GIT_ASKPASS", "");
        process.envs(env.iter().map(|(key, value)| (key, value)));
        (all, process)
    }
}

/// Where a streamed run is filed and what can end it: a clone has no repository yet.
/// In a repository the journal is `journal_entry`, which marks a pull stopped on conflicts.
pub(crate) struct Streamed<'a> {
    pub root: &'a std::path::Path,
    pub stop: Option<&'a NetworkStop>,
    pub journal: &'a dyn Fn(GitOutput),
}

impl Streamed<'_> {
    pub(crate) fn run(
        &self,
        process: std::process::Command,
        args: &[&str],
        on_line: impl FnMut(&str),
        silence: Duration,
    ) -> Result<()> {
        let (out, _) = self.stream_git(process, args, on_line, silence)?;
        self.settle(out, true)
    }

    pub(crate) fn run_recorded(
        &self,
        process: std::process::Command,
        args: &[&str],
        on_line: impl FnMut(&str),
        silence: Duration,
    ) -> Result<GitOutput> {
        let (out, _) = self.stream_git(process, args, on_line, silence)?;
        self.settle(out.clone(), true)?;
        Ok(out)
    }

    /// `run` for output that is parsed: the raw stdout, which the record trims, and a
    /// journal entry only on failure (R-280).
    pub(crate) fn run_read(
        &self,
        process: std::process::Command,
        args: &[&str],
        on_line: impl FnMut(&str),
        silence: Duration,
    ) -> Result<String> {
        let (out, stdout) = self.stream_git(process, args, on_line, silence)?;
        self.settle(out, false)?;
        Ok(stdout)
    }

    fn settle(&self, out: GitOutput, journal_success: bool) -> Result<()> {
        let succeeded = out.exit_code == Some(0);
        if journal_success || !succeeded {
            (self.journal)(out.clone());
        }
        if succeeded {
            return Ok(());
        }
        if self.cancelled() {
            return Err(GitError::Cancelled(out.command));
        }
        Err(out.into())
    }

    /// Delivered as it appears, not after the process exits. The record, and the whole
    /// stdout beside it.
    fn stream_git(
        &self,
        mut process: std::process::Command,
        args: &[&str],
        mut on_line: impl FnMut(&str),
        silence: Duration,
    ) -> Result<(GitOutput, String)> {
        let command = crate::redact_command(args);
        if self.cancelled() {
            return Err(GitError::Cancelled(command));
        }
        let started = Instant::now();
        tracing::info!(command = %command, "running git");

        let (child, _tracked) = crate::children::spawn(
            process
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped()),
        )
        .map_err(crate::runner::not_started)?;
        let stop = self.stop.cloned().unwrap_or_default();
        stop.hold(child.id());

        let heard = Arc::new(AtomicU64::new(0));
        let (finished, watching) = std::sync::mpsc::channel::<()>();
        let watchdog = {
            let (heard, stop) = (Arc::clone(&heard), stop.clone());
            std::thread::spawn(move || watch(&stop, started, &heard, silence, &watching))
        };
        // The end of the command is git's exit, not the end of its pipes: a hook's
        // background process keeps them open for as long as it lives (GR-04).
        let (mut stdout, mut stderr) = (Vec::new(), Progress::default());
        let status = crate::children::collect(child, Some(&stop), |is_stderr, chunk| {
            mark(&heard, started);
            if is_stderr {
                stderr.feed(chunk, &mut on_line);
            } else {
                stdout.extend_from_slice(chunk);
            }
        });
        let stderr_text = stderr.finish(&mut on_line);
        drop(finished);
        let stopped = watchdog.join().unwrap_or(false);
        let status = status?;
        let stdout = crate::output_text::decode(&stdout);

        let duration_ms = crate::runner::elapsed_ms(started);
        let mut result = GitOutput::record(
            self.root,
            command,
            status.code(),
            &stdout,
            &stderr_text,
            duration_ms,
        );
        if result.exit_code != Some(0) && self.cancelled() {
            result.summary = "Canceled by the user".to_owned();
            result.severity = crate::Severity::Warning;
        } else if stopped {
            result.summary = format!(
                "Stopped after {} s with no output from git\nIf the remote uses SSH, it may be waiting for a host key confirmation or a key passphrase: connect once from a terminal or add the key to ssh-agent",
                silence.as_secs()
            );
            tracing::warn!(command = %result.command, silence_s = silence.as_secs(), "stopped a silent network command");
        }
        Ok((result, stdout))
    }

    fn cancelled(&self) -> bool {
        self.stop.is_some_and(NetworkStop::is_stopped)
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
    stop: &NetworkStop,
    started: Instant,
    heard: &AtomicU64,
    silence: Duration,
    finished: &std::sync::mpsc::Receiver<()>,
) -> bool {
    loop {
        let last = Duration::from_millis(heard.load(Ordering::Relaxed));
        let quiet = started.elapsed().saturating_sub(last);
        if quiet >= silence {
            return stop.end_held();
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
            let line = crate::output_text::decode(&self.pending[..at])
                .trim_end()
                .to_owned();
            self.pending.drain(..=at);
            if !line.is_empty() {
                on_line(&crate::output_text::normalise(&line));
            }
        }
    }

    /// The last unterminated line, and everything as one text.
    fn finish(self, on_line: &mut impl FnMut(&str)) -> String {
        let rest = crate::output_text::decode(&self.pending);
        if !rest.trim().is_empty() {
            on_line(&crate::output_text::normalise(rest.trim_end()));
        }
        crate::output_text::decode(&self.all)
    }
}

/// Config handed over in the environment (`GIT_CONFIG_COUNT`), so that neither the command
/// line, the process list nor the journal ever holds it.
pub(crate) fn config_env(pairs: &[(String, String)]) -> Vec<(String, String)> {
    if pairs.is_empty() {
        return Vec::new();
    }
    let mut env = vec![("GIT_CONFIG_COUNT".to_owned(), pairs.len().to_string())];
    for (index, (key, value)) in pairs.iter().enumerate() {
        env.push((format!("GIT_CONFIG_KEY_{index}"), key.clone()));
        env.push((format!("GIT_CONFIG_VALUE_{index}"), value.clone()));
    }
    env
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    // A hook may echo the remote URL; the live line goes to the page, which the journal's
    // redaction does not reach (GR-07).
    #[test]
    fn a_progress_line_never_carries_a_token() {
        let mut seen = Vec::new();
        let mut progress = Progress::default();
        progress.feed(
            b"Pushing to https://oauth2:glpat-abc@gitlab.com/x.git\n",
            &mut |l| seen.push(l.to_owned()),
        );
        progress.feed(
            b"tail https://oauth2:glpat-abc@gitlab.com/z.git",
            &mut |_| {},
        );
        progress.finish(&mut |l| seen.push(l.to_owned()));
        let all = seen.join("\n");
        assert_eq!(seen.len(), 2, "{all}");
        assert!(all.contains("***") && !all.contains("glpat-abc"), "{all}");
    }

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
                &[],
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
        assert!(failure.summary.contains("ssh-agent"), "{failure:?}");
    }

    // VS Code's terminal hands Cogit its GIT_ASKPASS: the question went to VS Code and the
    // fetch waited 300 s for an answer (F-082). Set and empty, git skips core.askPass too.
    #[test]
    fn network_commands_run_without_askpass() {
        let f = test_fixtures::linear(1).unwrap();
        let repo = RepoHandle::open(f.path()).unwrap();

        let (_, process) = repo.network_command(&["fetch"], &[]);

        let askpass = process
            .get_envs()
            .find(|(key, _)| *key == "GIT_ASKPASS")
            .and_then(|(_, value)| value);
        assert_eq!(askpass, Some(std::ffi::OsStr::new("")));
    }

    #[test]
    fn config_entries_are_numbered_in_one_count() {
        let env = config_env(&[
            ("a.b".to_owned(), "1".to_owned()),
            ("c.d".to_owned(), "2".to_owned()),
        ]);

        let get = |key: &str| env.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
        assert_eq!(get("GIT_CONFIG_COUNT"), Some("2"));
        assert_eq!(get("GIT_CONFIG_KEY_1"), Some("c.d"));
        assert_eq!(get("GIT_CONFIG_VALUE_1"), Some("2"));
        assert!(config_env(&[]).is_empty());
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
