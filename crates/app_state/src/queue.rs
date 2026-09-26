//! One lane per repository, in the order the user asked.
//!
//! Two pushes at once are two `git push` processes fighting over the same ref lock, and
//! a commit racing a stage is two processes fighting over `index.lock`. Nothing here
//! throttles or drops: a click that arrived third runs third (P1.3).
//!
//! Reads do not come through here. They are allowed to overlap, and the ones the user has
//! outrun are cancelled instead — that is what `Cancellations` is for.

use crate::{AppEvent, RepoId};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::oneshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum OperationKind {
    Fetch,
    Pull,
    Push,
    Commit,
    Checkout,
    Branch,
    Merge,
    Rebase,
    Stage,
    Discard,
    Stash,
    Tag,
    Worktree,
    Submodule,
    Undo,
    /// In a lane of its own: the repository does not exist until it ends.
    Clone,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum OperationPhase {
    Queued,
    Running,
    Done,
}

/// What the toolbar and the queue indicator are told, at every phase.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub id: u32,
    /// `None` for work that belongs to no repository in particular.
    pub repo: Option<RepoId>,
    pub kind: OperationKind,
    pub label: String,
    pub phase: OperationPhase,
    /// Only ever `Some` once the phase is `Done`.
    pub success: Option<bool>,
}

#[derive(Debug, Default)]
struct Lane {
    running: Option<Operation>,
    waiting: VecDeque<(Operation, oneshot::Sender<()>)>,
}

/// Keyed by `OpenRepo::lane`: a linked worktree or a submodule writes into the files of
/// the repository it belongs to, so it waits in that repository's lane (CC-007).
#[derive(Debug, Default)]
pub struct Queue {
    lanes: Mutex<HashMap<PathBuf, Lane>>,
    next_id: AtomicU32,
    /// A lane emptied; whoever waits for the whole queue looks again.
    emptied: tokio::sync::Notify,
}

impl Queue {
    /// Takes a place in the repository's lane and says whether the turn is now.
    ///
    /// Sync, under one lock, at the first poll of the command: its place is fixed then and
    /// kept to the end. Tauri spawns each async command on the multi-threaded runtime, so
    /// two calls sent without awaiting the first may reach this in either order (R-513);
    /// clicks, each awaited, run in the order they landed.
    fn admit(
        &self,
        lane: &Path,
        repo: Option<RepoId>,
        kind: OperationKind,
        label: String,
    ) -> (Operation, Option<oneshot::Receiver<()>>) {
        let mut lanes = self.lanes.lock();
        let lane = lanes.entry(lane.to_path_buf()).or_default();
        let mut operation = Operation {
            id: self.next_id.fetch_add(1, Ordering::Relaxed),
            repo,
            kind,
            label,
            phase: OperationPhase::Queued,
            success: None,
        };

        if lane.running.is_none() {
            operation.phase = OperationPhase::Running;
            lane.running = Some(operation.clone());
            return (operation, None);
        }

        let (wake, wait) = oneshot::channel();
        lane.waiting.push_back((operation.clone(), wake));
        (operation, Some(wait))
    }

    /// Hands the lane to whoever is next in it.
    fn release(&self, key: &Path) {
        let mut lanes = self.lanes.lock();
        let Some(lane) = lanes.get_mut(key) else {
            return;
        };
        lane.running = None;

        while let Some((mut next, wake)) = lane.waiting.pop_front() {
            next.phase = OperationPhase::Running;
            lane.running = Some(next);
            if wake.send(()).is_ok() {
                return;
            }
            // Its caller is gone — the command was dropped before its turn came.
            lane.running = None;
        }
        lanes.remove(key);
        drop(lanes);
        self.emptied.notify_waiters();
    }

    /// Everything in flight, running first: what a panel that just opened has missed.
    #[must_use]
    pub fn snapshot(&self) -> Vec<Operation> {
        let lanes = self.lanes.lock();
        let mut out: Vec<Operation> = lanes
            .values()
            .filter_map(|lane| lane.running.clone())
            .collect();
        out.extend(
            lanes
                .values()
                .flat_map(|lane| lane.waiting.iter().map(|(operation, _)| operation.clone())),
        );
        out.sort_by_key(|operation| operation.id);
        out
    }
}

impl crate::AppState {
    /// Waits for this repository's lane, announcing itself at every phase.
    ///
    /// The permit must outlive the work: dropping it is what lets the next one start.
    pub async fn enqueue(
        &self,
        repo: RepoId,
        kind: OperationKind,
        label: &str,
    ) -> OperationPermit<'_> {
        self.enqueue_in(self.lane_of(repo), Some(repo), kind, label)
            .await
    }

    /// `enqueue` for work that belongs to no open repository: a clone waits in the lane of
    /// the folder it goes to.
    pub async fn enqueue_detached(
        &self,
        lane: PathBuf,
        kind: OperationKind,
        label: &str,
    ) -> OperationPermit<'_> {
        self.enqueue_in(lane, None, kind, label).await
    }

    async fn enqueue_in(
        &self,
        lane: PathBuf,
        repo: Option<RepoId>,
        kind: OperationKind,
        label: &str,
    ) -> OperationPermit<'_> {
        let (operation, wait) = self.queue.admit(&lane, repo, kind, label.to_owned());
        self.emit(AppEvent::Operation(operation.clone()));

        let mut operation = operation;
        if let Some(wait) = wait {
            let _ = wait.await;
            operation.phase = OperationPhase::Running;
            self.emit(AppEvent::Operation(operation.clone()));
        }

        OperationPermit {
            state: self,
            operation,
            lane,
            settled: false,
        }
    }

    /// Everything queued or running, for a panel that has just been opened again (P1.5).
    #[must_use]
    pub fn operations(&self) -> Vec<Operation> {
        self.queue.snapshot()
    }

    #[must_use]
    pub fn session_end_blocker(&self) -> Option<String> {
        match self.queue.snapshot().len() {
            0 => None,
            1 => Some("1 operation is still running".to_owned()),
            n => Some(format!("{n} operations are still running")),
        }
    }

    /// Once nothing is queued or running. Registered before each look, so a lane that
    /// empties between the look and the wait is not missed.
    pub async fn until_idle(&self) {
        loop {
            let mut emptied = std::pin::pin!(self.queue.emptied.notified());
            emptied.as_mut().enable();
            if self.queue.snapshot().is_empty() {
                return;
            }
            emptied.await;
        }
    }
}

/// Holds one repository's lane until the work is over.
#[derive(Debug)]
pub struct OperationPermit<'a> {
    state: &'a crate::AppState,
    operation: Operation,
    lane: PathBuf,
    settled: bool,
}

impl OperationPermit<'_> {
    pub fn finish(mut self, success: bool) {
        self.settle(success);
    }

    #[must_use]
    pub fn id(&self) -> u32 {
        self.operation.id
    }

    fn settle(&mut self, success: bool) {
        if self.settled {
            return;
        }
        self.settled = true;
        self.operation.phase = OperationPhase::Done;
        self.operation.success = Some(success);
        // Released first: whoever hears `Done` asks the queue what is left, and the
        // session-end reason was kept for an operation already over (R-168).
        self.state.queue.release(&self.lane);
        self.state.emit(AppEvent::Operation(self.operation.clone()));
    }
}

impl Drop for OperationPermit<'_> {
    /// A command that returned early, or panicked, must not wedge the lane shut.
    fn drop(&mut self) {
        self.settle(false);
    }
}

impl OperationKind {
    /// What the toolbar calls it while it runs.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Fetch => "Fetching",
            Self::Pull => "Pulling",
            Self::Push => "Pushing",
            Self::Commit => "Committing",
            Self::Checkout => "Checking out",
            Self::Branch => "Updating branches",
            Self::Merge => "Merging",
            Self::Rebase => "Rebasing",
            Self::Stage => "Staging",
            Self::Discard => "Discarding",
            Self::Stash => "Stashing",
            Self::Tag => "Tagging",
            Self::Worktree => "Updating worktrees",
            Self::Submodule => "Updating submodules",
            Self::Undo => "Undoing",
            Self::Clone => "Cloning",
            Self::Other => "Working",
        }
    }
}
