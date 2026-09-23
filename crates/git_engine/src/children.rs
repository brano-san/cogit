//! Every process Cogit started and has not reaped yet, so that exiting can stop them (R-170).

use std::collections::BTreeSet;
use std::io::Write as _;
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

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

pub(crate) fn output(command: &mut Command) -> std::io::Result<Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let (child, _tracked) = spawn(command)?;
    child.wait_with_output()
}

/// `output` with `input` on stdin. Written from a thread of its own: a child that fills
/// its stdout before reading all of stdin would otherwise wait on us forever.
pub(crate) fn output_fed(command: &mut Command, input: &[u8]) -> std::io::Result<Output> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let (mut child, _tracked) = spawn(command)?;
    let stdin = child.stdin.take();
    std::thread::scope(|scope| {
        scope.spawn(move || {
            // A child that quit early says why in its exit code; the broken pipe adds nothing.
            let _ = stdin.map(|mut pipe| pipe.write_all(input));
        });
        child.wait_with_output()
    })
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
fn stop_tree(pid: u32) -> std::io::Result<()> {
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
fn stop_tree(pid: u32) -> std::io::Result<()> {
    let status = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("kill exited with {status}")))
    }
}
