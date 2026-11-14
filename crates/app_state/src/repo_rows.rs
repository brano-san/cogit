//! Rows of the Repositories list not on screen, by folder: a closed one has no `RepoId`.

use git_engine::{GitError, RepoHandle, Submodule};
use std::path::Path;

/// `parent` is a key from the top, `""` for the top (R-352).
pub fn submodule_outline(root: &Path, parent: &str) -> Result<Vec<Submodule>, GitError> {
    let handle = if parent.is_empty() {
        RepoHandle::open_root(root)?
    } else {
        RepoHandle::open_exact(&root.join(parent))?
    };
    handle.submodule_outline()
}

#[must_use]
pub fn pulse(root: &Path) -> git_engine::RepoPulse {
    git_engine::pulse(root)
}

impl crate::AppState {
    /// Asks the server, writes nothing; a failure is logged and means "unknown" (R-354).
    /// Sends what is stored for the host, never asks (F-082).
    pub fn pull_probe(&self, root: &Path) -> Result<Option<bool>, GitError> {
        let open = self.find_by_root(root);
        let turn = open.map(|repo| self.background_turn(repo));
        let _quiet = open.map(|repo| self.quiet_briefly(repo));
        // Cancel in the footer stops it, here or in the agent (R-946).
        let run = turn.as_ref().map(|turn| self.network_stop(turn.operation));
        let stop = run.as_ref().map(crate::network::NetworkRun::token);
        self.pull_probe_until(root, stop.as_ref())
    }

    /// `pull_probe` without a turn of the lane: the agent's side of it, `stop` its call's.
    pub(crate) fn pull_probe_until(
        &self,
        root: &Path,
        stop: Option<&git_engine::NetworkStop>,
    ) -> Result<Option<bool>, GitError> {
        let probed = self
            .on_host_of_until(root, stop, |root| crate::backend::Request::PullProbe {
                root,
            })
            .unwrap_or_else(|| {
                RepoHandle::open_root(root)
                    .and_then(|handle| handle.pull_probe_until(self.stored(), stop))
            });
        if let Err(err) = &probed {
            tracing::warn!(root = %root.display(), error = %err, "pull probe failed");
        }
        probed
    }

    pub fn submodule_outline(&self, root: &Path, parent: &str) -> Result<Vec<Submodule>, GitError> {
        self.on_host_of(root, |root| crate::backend::Request::SubmoduleOutline {
            root,
            parent: parent.to_owned(),
        })
        .unwrap_or_else(|| submodule_outline(root, parent))
    }

    pub(crate) fn pulse_of(&self, root: &Path) -> git_engine::RepoPulse {
        self.on_host_of(root, |root| crate::backend::Request::Pulse { root })
            .map(|pulse| {
                pulse.unwrap_or_else(|err| {
                    tracing::error!(error = ?err, context = "pulse in the agent");
                    git_engine::RepoPulse::default()
                })
            })
            .unwrap_or_else(|| pulse(root))
    }
}
