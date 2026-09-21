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

#[derive(Debug, Default)]
pub struct Queue {
    lanes: Mutex<HashMap<RepoId, Lane>>,
    next_id: AtomicU32,
}

impl Queue {
    /// Takes a place in the repository's lane and says whether the turn is now.
    ///
    /// Sync on purpose: the place in the queue is taken when the command is called, not
    /// when its future happens to be polled, so three clicks run in the order they landed.
    fn admit(
        &self,
        repo: RepoId,
        kind: OperationKind,
        label: String,
    ) -> (Operation, Option<oneshot::Receiver<()>>) {
        let mut lanes = self.lanes.lock();
        let lane = lanes.entry(repo).or_default();
        let mut operation = Operation {
            id: self.next_id.fetch_add(1, Ordering::Relaxed),
            repo: Some(repo),
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
    fn release(&self, repo: RepoId) {
        let mut lanes = self.lanes.lock();
        let Some(lane) = lanes.get_mut(&repo) else {
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
        lanes.remove(&repo);
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
        let (operation, wait) = self.queue.admit(repo, kind, label.to_owned());
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
            settled: false,
        }
    }

    /// Everything queued or running, for a panel that has just been opened again (P1.5).
    #[must_use]
    pub fn operations(&self) -> Vec<Operation> {
        self.queue.snapshot()
    }
}

/// Holds one repository's lane until the work is over.
#[derive(Debug)]
pub struct OperationPermit<'a> {
    state: &'a crate::AppState,
    operation: Operation,
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
        self.state.emit(AppEvent::Operation(self.operation.clone()));
        if let Some(repo) = self.operation.repo {
            self.state.queue.release(repo);
        }
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
            Self::Other => "Working",
        }
    }
}
