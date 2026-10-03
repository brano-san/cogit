//! Remote ▸ Submodule, Subtree and LFS (#45, #46) and Repository ▸ Settings (#42).

use crate::watching::Quiet;
use crate::{AppState, RepoId};
use git_engine::{
    GitError, LfsOp, NetworkStop, RepoSetting, RepoSettingChange, SubmoduleOp, SubtreeOp,
};

/// What `guard` asks of an op; the three kinds spell it in methods of their own.
trait TalksToAServer {
    fn talks_to_a_server(&self) -> bool;
}

impl TalksToAServer for SubmoduleOp {
    fn talks_to_a_server(&self) -> bool {
        SubmoduleOp::talks_to_a_server(*self)
    }
}

impl TalksToAServer for SubtreeOp {
    fn talks_to_a_server(&self) -> bool {
        SubtreeOp::talks_to_a_server(self)
    }
}

impl TalksToAServer for LfsOp {
    fn talks_to_a_server(&self) -> bool {
        LfsOp::talks_to_a_server(self)
    }
}

impl AppState {
    /// A talk with a server runs for minutes and writes at its end, so it is only silenced
    /// briefly; a hold would keep the edits made meanwhile off the panels (R-445). A local
    /// op is held quiet to its end, so its own writes do not come back as disk changes.
    fn guard(&self, repo: RepoId, op: &impl TalksToAServer) -> Quiet<'_> {
        if op.talks_to_a_server() {
            self.quiet_briefly(repo)
        } else {
            self.quiet(repo)
        }
    }

    pub fn submodule_op(
        &self,
        repo: RepoId,
        op: SubmoduleOp,
        paths: &[String],
        stop: &NetworkStop,
    ) -> Result<(), GitError> {
        let _quiet = self.guard(repo, &op);
        self.handle(repo)?
            .with_stop(stop.clone())
            .submodule_op(op, paths)
    }

    pub fn add_submodule(
        &self,
        repo: RepoId,
        url: &str,
        path: &str,
        branch: Option<&str>,
        stop: &NetworkStop,
    ) -> Result<(), GitError> {
        let _quiet = self.quiet_briefly(repo);
        self.handle(repo)?
            .with_stop(stop.clone())
            .add_submodule(url, path, branch)
    }

    pub fn subtree_op(
        &self,
        repo: RepoId,
        op: &SubtreeOp,
        stop: &NetworkStop,
    ) -> Result<(), GitError> {
        let _quiet = self.guard(repo, op);
        self.handle(repo)?.with_stop(stop.clone()).subtree_op(op)
    }

    pub fn subtree_prefixes(&self, repo: RepoId) -> Result<Vec<String>, GitError> {
        self.handle(repo)?.subtree_prefixes()
    }

    pub fn lfs_op(&self, repo: RepoId, op: &LfsOp, stop: &NetworkStop) -> Result<(), GitError> {
        let _quiet = self.guard(repo, op);
        self.handle(repo)?.with_stop(stop.clone()).lfs_op(op)
    }

    pub fn repo_settings(&self, repo: RepoId) -> Result<Vec<RepoSetting>, GitError> {
        Ok(self.handle(repo)?.repo_settings())
    }

    pub fn write_repo_settings(
        &self,
        repo: RepoId,
        changes: &[RepoSettingChange],
    ) -> Result<(), GitError> {
        self.handle(repo)?.write_repo_settings(changes)
    }
}

#[cfg(test)]
mod tests {
    use crate::AppState;
    use git_engine::{LfsOp, SubmoduleOp, SubtreeOp};

    /// What `guard` is told: only these are silenced briefly, the rest held quiet.
    #[test]
    fn only_what_talks_to_a_server_is_silenced_briefly() {
        for op in [SubmoduleOp::Initialize, SubmoduleOp::Reset] {
            assert!(op.talks_to_a_server(), "{op:?}");
        }
        for op in [
            SubmoduleOp::Synchronize,
            SubmoduleOp::Deactivate,
            SubmoduleOp::Deinit,
            SubmoduleOp::Unregister,
        ] {
            assert!(!op.talks_to_a_server(), "{op:?}");
        }

        let (prefix, repository, reference) = ("p".to_owned(), "r".to_owned(), "main".to_owned());
        let add = SubtreeOp::Add {
            prefix: prefix.clone(),
            repository: repository.clone(),
            reference: reference.clone(),
            squash: false,
        };
        let pull = SubtreeOp::Merge {
            prefix: prefix.clone(),
            repository: Some(repository),
            reference: reference.clone(),
            squash: false,
        };
        let local = SubtreeOp::Merge {
            prefix: prefix.clone(),
            repository: None,
            reference: reference.clone(),
            squash: false,
        };
        let split = SubtreeOp::Split {
            prefix: prefix.clone(),
            branch: "b".to_owned(),
            rejoin: false,
        };
        let reset = SubtreeOp::Reset { prefix, reference };
        assert!(add.talks_to_a_server() && pull.talks_to_a_server());
        assert!(!local.talks_to_a_server());
        assert!(!split.talks_to_a_server() && !reset.talks_to_a_server());

        let paths = vec!["a".to_owned()];
        assert!(
            LfsOp::Lock {
                paths: paths.clone()
            }
            .talks_to_a_server()
        );
        assert!(LfsOp::Unlock { paths }.talks_to_a_server());
        for op in [
            LfsOp::Install,
            LfsOp::Prune,
            LfsOp::Track {
                pattern: "*.bin".to_owned(),
            },
        ] {
            assert!(!op.talks_to_a_server(), "{op:?}");
        }
    }

    /// The predicate above only matters through `guard`: an op that talks to a server is
    /// not held, a local one is, whatever the kind of op.
    #[test]
    fn the_guard_holds_a_local_op_and_only_silences_an_online_one() {
        let f = test_fixtures::linear(1).unwrap();
        let state = AppState::new();
        let repo = state.open_repository(f.path()).unwrap().repo;
        // Shown, or there is no watcher to hold.
        state.show_repository(Some(repo));
        let (prefix, reference) = ("p".to_owned(), "main".to_owned());
        let online_subtree = SubtreeOp::Add {
            prefix: prefix.clone(),
            repository: "r".to_owned(),
            reference: reference.clone(),
            squash: false,
        };
        let local_subtree = SubtreeOp::Reset { prefix, reference };

        assert!(!state.guard(repo, &SubmoduleOp::Initialize).is_held());
        assert!(state.guard(repo, &SubmoduleOp::Synchronize).is_held());
        assert!(!state.guard(repo, &online_subtree).is_held());
        assert!(state.guard(repo, &local_subtree).is_held());
        let lock = LfsOp::Lock {
            paths: vec!["a".to_owned()],
        };
        assert!(!state.guard(repo, &lock).is_held());
        assert!(state.guard(repo, &LfsOp::Install).is_held());
    }
}
