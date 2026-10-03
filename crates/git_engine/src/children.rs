//! Every process Cogit started and has not reaped yet, so that exiting can stop them (R-170).

use crate::network::NetworkStop;
use std::collections::BTreeSet;
use std::io::{Read, Write as _};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

static RUNNING: Mutex<BTreeSet<u32>> = Mutex::new(BTreeSet::new());
static STOPPING: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
pub(crate) struct Tracked(u32);

impl Drop for Tracked {
    fn drop(&mut self) {
        RUNNING
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.0);
    }
}

/// `Command::spawn`, remembered; refused once exiting has begun.
pub(crate) fn spawn(command: &mut Command) -> std::io::Result<(Child, Tracked)> {
    if STOPPING.load(Ordering::SeqCst) {
        return Err(exiting());
    }
    // Its own group, so that `stop_tree` reaches what git starts (`sh -c`, ssh, helpers).
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    let child = command.spawn()?;
    let late = {
        let mut running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
        running.insert(child.id());
        STOPPING.load(Ordering::SeqCst)
    };
    let tracked = Tracked(child.id());
    // `stop_all` ran between the check and the insert and could not have seen this one.
    if late {
        let _ = stop_tree(child.id());
        return Err(exiting());
    }
    Ok((child, tracked))
}

/// How long a command's output is waited for once git has exited: what a process it
/// started still holds open is not part of the command (GR-04).
const AFTER_EXIT: Duration = Duration::from_secs(2);

/// How often a running command is checked for having exited while its pipes are quiet.
const POLL: Duration = Duration::from_millis(100);

pub(crate) fn output(command: &mut Command) -> std::io::Result<Output> {
    output_within(command, None, None)
}

/// `output` with `input` on stdin. Written from a thread of its own, left behind if a
/// process git started keeps the pipe: a child that fills its stdout before reading all
/// of stdin would otherwise wait on us forever.
pub(crate) fn output_fed(command: &mut Command, input: &[u8]) -> std::io::Result<Output> {
    output_within(command, Some(input), None)
}

fn output_within(
    command: &mut Command,
    input: Option<&[u8]>,
    stop: Option<&NetworkStop>,
) -> std::io::Result<Output> {
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let (mut child, _tracked) = spawn(command)?;
    if let (Some(input), Some(mut pipe)) = (input, child.stdin.take()) {
        let input = input.to_vec();
        std::thread::spawn(move || {
            // A child that quit early says why in its exit code; the broken pipe adds nothing.
            let _ = pipe.write_all(&input);
        });
    }
    if let Some(stop) = stop {
        stop.hold(child.id());
    }
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let status = collect(child, stop, |is_stderr, chunk| {
        if is_stderr { &mut stderr } else { &mut stdout }.extend_from_slice(chunk);
    })?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

/// Waits for the process itself, hands `on_chunk` what it writes meanwhile, and gives what
/// is still being written after it exited `AFTER_EXIT`. The readers end on their own at EOF.
///
/// `stop` was told the pid (`hold`) and is told it is gone: the process is reaped only under
/// its lock, so that nobody ends a pid that has gone to another process.
pub(crate) fn collect(
    mut child: Child,
    stop: Option<&NetworkStop>,
    mut on_chunk: impl FnMut(bool, &[u8]),
) -> std::io::Result<ExitStatus> {
    let (send, receive) = mpsc::channel::<(bool, Vec<u8>)>();
    if let Some(pipe) = child.stdout.take() {
        read_in_thread(pipe, false, send.clone());
    }
    if let Some(pipe) = child.stderr.take() {
        read_in_thread(pipe, true, send.clone());
    }
    drop(send);

    let mut status = None;
    let mut give_up = None;
    loop {
        match receive.recv_timeout(POLL) {
            Ok((is_stderr, chunk)) => on_chunk(is_stderr, &chunk),
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        if status.is_none() {
            status = match stop {
                Some(stop) => stop.reap(&mut child),
                None => child.try_wait(),
            }
            .inspect_err(|_| {
                if let Some(stop) = stop {
                    stop.release();
                }
            })?;
            give_up = status.map(|_| Instant::now() + AFTER_EXIT);
        }
        if give_up.is_some_and(|at| Instant::now() >= at) {
            tracing::warn!("git exited; a process it started still holds its output");
            break;
        }
    }
    if let Some(status) = status {
        return Ok(status);
    }
    if let Some(stop) = stop {
        stop.release();
    }
    child.wait()
}

fn read_in_thread(
    mut pipe: impl Read + Send + 'static,
    is_stderr: bool,
    send: mpsc::Sender<(bool, Vec<u8>)>,
) {
    std::thread::spawn(move || {
        let mut chunk = [0_u8; 8192];
        while let Ok(read @ 1..) = pipe.read(&mut chunk) {
            if send.send((is_stderr, chunk[..read].to_vec())).is_err() {
                break;
            }
        }
    });
}

fn exiting() -> std::io::Error {
    std::io::Error::other("Cogit is exiting")
}

#[must_use]
pub fn running() -> usize {
    RUNNING.lock().unwrap_or_else(PoisonError::into_inner).len()
}

/// Stops every tracked process tree and refuses new ones; returns how many were stopped.
pub fn stop_all() -> usize {
    let pids: Vec<u32> = {
        let running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
        STOPPING.store(true, Ordering::SeqCst);
        running.iter().copied().collect()
    };
    for pid in &pids {
        if let Err(err) = stop_tree(*pid) {
            tracing::warn!(pid, error = %err, "cannot stop a git process on exit");
        }
    }
    if !pids.is_empty() {
        tracing::info!(
            stopped = pids.len(),
            "stopped the git processes still running"
        );
    }
    pids.len()
}

#[cfg(windows)]
pub(crate) fn stop_tree(pid: u32) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt as _;
    // `/T` for sh, ssh and git-remote-https; `/F` since a windowless process cannot close.
    let status = Command::new("taskkill")
        .creation_flags(crate::runner::CREATE_NO_WINDOW)
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .output()?
        .status;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "taskkill exited with {status}"
        )))
    }
}

#[cfg(not(windows))]
pub(crate) fn stop_tree(pid: u32) -> std::io::Result<()> {
    let status = Command::new("kill")
        .args(["-TERM", "--", &format!("-{pid}")])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("kill exited with {status}")))
    }
}
