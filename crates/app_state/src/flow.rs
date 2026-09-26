//! Git Flow over the engine's flow commands, with Undo for a finished branch (M8).

use crate::{AppState, Recovery, RepoId};

impl AppState {
    pub fn flow_status(
        &self,
        repo: RepoId,
    ) -> Result<git_engine::FlowStatus, git_engine::GitError> {
        self.handle(repo)?.flow_status()
    }

    pub fn flow_init(
        &self,
        repo: RepoId,
        config: &git_engine::FlowConfig,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.flow_init(config)
    }

    pub fn flow_start(
        &self,
        repo: RepoId,
        kind: git_engine::FlowKind,
        name: &str,
    ) -> Result<String, git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.flow_start(kind, name)
    }

    /// Destructive: the branch is deleted once it is folded back, so the head it had
    /// is kept for Undo.
    pub fn flow_finish(
        &self,
        repo: RepoId,
        kind: git_engine::FlowKind,
        name: &str,
        tag: Option<&str>,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let status = handle.flow_status()?;
        let full = match kind {
            git_engine::FlowKind::Feature => format!("{}{name}", status.config.feature),
            git_engine::FlowKind::Release => format!("{}{name}", status.config.release),
            git_engine::FlowKind::Hotfix => format!("{}{name}", status.config.hotfix),
        };
        // From the branch list, as `delete_branch` does: `rev-parse <name>` would prefer a
        // tag of the same name.
        let finished = handle
            .branches_without_divergence()?
            .into_iter()
            .find(|branch| branch.name == full);

        handle.flow_finish(kind, name, tag)?;

        self.record(
            repo,
            format!("Finish {full}"),
            finished.map_or(Recovery::None, |branch| Recovery::Branch {
                name: full,
                oid: branch.oid,
                upstream: branch.upstream,
            }),
        );
        Ok(())
    }
}
