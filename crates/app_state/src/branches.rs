//! Checkout, branches and tags; a deleted branch or tag is recorded for Undo (M5).

use crate::{AppState, Recovery, RepoId};

impl AppState {
    pub fn checkout(
        &self,
        repo: RepoId,
        target: &git_engine::CheckoutTarget,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.checkout(target)?;

        self.record(
            repo,
            format!("Check out {}", target.described()),
            Recovery::None,
        );
        Ok(())
    }

    pub fn create_branch(
        &self,
        repo: RepoId,
        name: &str,
        start: Option<&str>,
        switch: bool,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.create_branch(name, start, switch)
    }

    pub fn rename_branch(
        &self,
        repo: RepoId,
        from: &str,
        to: &str,
        force: bool,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.rename_branch(from, to, force)
    }

    pub fn set_upstream(
        &self,
        repo: RepoId,
        branch: &str,
        upstream: Option<&str>,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.set_upstream(branch, upstream)
    }

    /// Reaches the server, so it is tracked and journalled like any other network call.
    pub fn delete_remote_branch(
        &self,
        repo: RepoId,
        remote: &str,
        branch: &str,
    ) -> Result<git_engine::RemoteDeletion, git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.delete_remote_branch(remote, branch)
    }

    pub fn delete_branch(
        &self,
        repo: RepoId,
        name: &str,
        force: bool,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let deleted = handle
            .branches_without_divergence()?
            .into_iter()
            .find(|branch| branch.name == name);
        handle.delete_branch(name, force)?;

        self.record(
            repo,
            format!("Delete branch {name}"),
            deleted.map_or(Recovery::None, |branch| Recovery::Branch {
                name: name.to_owned(),
                oid: branch.oid,
                upstream: branch.upstream,
            }),
        );
        Ok(())
    }

    pub fn create_tag(
        &self,
        repo: RepoId,
        request: &git_engine::TagRequest,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.create_tag(request)
    }

    pub fn delete_tag(&self, repo: RepoId, name: &str) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        let handle = self.handle(repo)?;
        let oid = handle.tag_target(name);
        handle.delete_tag(name)?;
        self.record(
            repo,
            format!("Delete tag {name}"),
            oid.map_or(Recovery::None, |oid| Recovery::Tag {
                name: name.to_owned(),
                oid,
            }),
        );
        Ok(())
    }
}
