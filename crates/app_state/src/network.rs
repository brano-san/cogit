//! The commands that talk to a remote, signed in as `signed_in` decides (F-082).

use crate::{AppState, RepoId, SignIn, credential_target, host_input};
use git_engine::{
    AuthRequired, Credential, CredentialTrouble, GitError, NetworkStop, PullOutcome, Pushed,
};
use std::cell::{Cell, RefCell};

pub(crate) type Credentials<'a> = &'a dyn Fn(&str) -> Option<Credential>;

thread_local! {
    /// Set while `signed_in` runs: the journal is written on this thread, before the page
    /// learns of the refusal and opens the Sign in dialog.
    static SIGNING_IN: Cell<bool> = const { Cell::new(false) };
}

/// A refusal the Sign in dialog asks about is no error notice as well. A host it cannot
/// sign in to still reaches the Errors window: the rejected call carries no `authRequired`.
pub(crate) fn asks_sign_in(trouble: Option<CredentialTrouble>) -> bool {
    SIGNING_IN.get()
        && matches!(
            trouble,
            Some(CredentialTrouble::NoPrompt | CredentialTrouble::Refused)
        )
}

/// A network command running as one queue operation; `cancel_network` finds it until
/// this is dropped.
#[derive(Debug)]
pub struct NetworkRun<'a> {
    state: &'a AppState,
    operation: u32,
    stop: NetworkStop,
}

impl NetworkRun<'_> {
    #[must_use]
    pub fn token(&self) -> NetworkStop {
        self.stop.clone()
    }
}

impl Drop for NetworkRun<'_> {
    fn drop(&mut self) {
        self.state.network_runs.lock().remove(&self.operation);
    }
}

impl AppState {
    #[must_use]
    pub fn network_stop(&self, operation: u32) -> NetworkRun<'_> {
        let stop = NetworkStop::default();
        self.network_runs.lock().insert(operation, stop.clone());
        NetworkRun {
            state: self,
            operation,
            stop,
        }
    }

    /// Stops the fetch, pull or push running as `operation`. `false` when there is none:
    /// it has finished, is still waiting in the queue, is not a network command, or was
    /// asked to stop already.
    pub fn cancel_network(&self, operation: u32) -> bool {
        let stop = self.network_runs.lock().get(&operation).cloned();
        let stopped = stop.is_some_and(|stop| stop.stop());
        tracing::info!(operation, stopped, "network cancel requested");
        stopped
    }

    pub(crate) fn local_remotes(&self, repo: RepoId) -> Result<Vec<String>, git_engine::GitError> {
        self.handle(repo)?.remotes()
    }

    pub(crate) fn local_fetch(
        &self,
        repo: RepoId,
        remote: &str,
        stop: &NetworkStop,
        sign_in: Option<&SignIn>,
        on_line: impl FnMut(&str),
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet_briefly(repo);
        self.signed_in(sign_in, |credential| {
            let handle = self.handle(repo)?.with_stop(stop.clone());
            handle.fetch(remote, credential, on_line)
        })
    }

    /// On a branch that tracks nothing there is nothing to merge: Pull fetches every
    /// remote and says nothing more (R-552). On a detached HEAD it only fetches `remote`
    /// and never moves HEAD (F-710). Own commits are merged or rebased as the repository
    /// remembers for the Pull dialog (R-841).
    pub(crate) fn local_pull(
        &self,
        repo: RepoId,
        remote: &str,
        autostash: bool,
        stop: &NetworkStop,
        sign_in: Option<&SignIn>,
        on_line: impl FnMut(&str),
    ) -> Result<PullOutcome, git_engine::GitError> {
        let _quiet = self.quiet_briefly(repo);
        self.signed_in(sign_in, |credential| {
            let handle = self.handle(repo)?.with_stop(stop.clone());
            if matches!(handle.head()?, git_engine::Head::Detached { .. }) {
                handle.fetch(remote, credential, on_line)?;
                return Ok(PullOutcome::FetchedDetached);
            }
            if handle.head_tracks_nothing() {
                handle.fetch_all(credential, on_line)?;
                return Ok(PullOutcome::Pulled);
            }
            // Listed before the pull: a submodule the user deinitialised stays that way (#42).
            let known = handle
                .wants_new_submodules()
                .then(|| handle.submodule_paths());
            let before = handle.head()?;
            let method = handle
                .network_defaults()
                .map(|defaults| defaults.pull_method)
                .inspect_err(|err| {
                    tracing::error!(error = ?err, context = "pull method unreadable, rebasing");
                })
                .unwrap_or_default();
            let options = git_engine::PullOptions {
                method,
                autostash,
                ..git_engine::PullOptions::default()
            };
            let result = handle.pull_options(remote, options, credential, on_line);
            self.record_move(
                repo,
                &handle,
                before,
                format!("Pull from {remote}"),
                &result,
            );
            let outcome = PullOutcome::from(&result?);
            if outcome == PullOutcome::StashNeeded {
                return Ok(outcome);
            }
            if let Some(known) = known {
                handle.init_submodules_added_since(&known)?;
            }
            Ok(outcome)
        })
    }

    /// On a detached HEAD there is no current branch: every branch ahead of where it
    /// pushes goes instead, never forced, and `Some` says what went — empty when nothing
    /// could, and then nothing ran; tags are not pushed (F-710).
    pub(crate) fn local_push(
        &self,
        repo: RepoId,
        remote: &str,
        force: bool,
        stop: &NetworkStop,
        sign_in: Option<&SignIn>,
        on_line: impl FnMut(&str),
    ) -> Result<Option<Pushed>, git_engine::GitError> {
        let _quiet = self.quiet_briefly(repo);
        self.signed_in(sign_in, |credential| {
            let handle = self.handle(repo)?.with_stop(stop.clone());
            if matches!(handle.head()?, git_engine::Head::Detached { .. }) {
                return handle.push_pushable(credential, on_line).map(Some);
            }
            handle.push(remote, None, force, credential, on_line)?;
            Ok(None)
        })
    }

    /// Remote ▸ Fetch More; `false` when nothing new came (R-553).
    pub(crate) fn local_fetch_more(
        &self,
        repo: RepoId,
        remote: &str,
        stop: &NetworkStop,
        sign_in: Option<&SignIn>,
        on_line: impl FnMut(&str),
    ) -> Result<bool, git_engine::GitError> {
        let _quiet = self.quiet_briefly(repo);
        self.signed_in(sign_in, |credential| {
            let handle = self.handle(repo)?.with_stop(stop.clone());
            handle.fetch_more(remote, credential, on_line)
        })
    }

    pub(crate) fn local_fetch_depth(
        &self,
        repo: RepoId,
        remote: &str,
        depth: u32,
        stop: &NetworkStop,
        sign_in: Option<&SignIn>,
        on_line: impl FnMut(&str),
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet_briefly(repo);
        self.signed_in(sign_in, |credential| {
            let handle = self.handle(repo)?.with_stop(stop.clone());
            handle.fetch_depth(remote, depth, credential, on_line)
        })
    }

    /// Pull ▸ Delete merged branches after Pull (#26). Each deletion is journalled, so
    /// Undo brings a branch back; one that refuses is logged and the rest still go.
    pub(crate) fn local_delete_merged_branches(
        &self,
        repo: RepoId,
    ) -> Result<Vec<String>, git_engine::GitError> {
        let names = self.handle(repo)?.merged_gone_branches()?;
        let mut deleted = Vec::with_capacity(names.len());
        for name in names {
            match self.delete_branch(repo, &name, false) {
                Ok(git_engine::BranchDeletion::Deleted) => deleted.push(name),
                Ok(git_engine::BranchDeletion::NotFullyMerged) => {
                    tracing::warn!(branch = %name, "kept a branch that is not fully merged");
                }
                Err(err) => {
                    tracing::error!(error = ?err, branch = %name, context = "failed to delete a merged branch");
                }
            }
        }
        Ok(deleted)
    }

    /// One refspec to one remote: Push To, Push Up To and pushing a ref that is not HEAD.
    /// `track`: the branch pushed tracks what it becomes there (R-550).
    #[expect(
        clippy::too_many_arguments,
        reason = "one IPC command's arguments, passed through as they came"
    )]
    pub(crate) fn local_push_to(
        &self,
        repo: RepoId,
        remote: &str,
        refspec: &str,
        track: bool,
        stop: &NetworkStop,
        sign_in: Option<&SignIn>,
        on_line: impl FnMut(&str),
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet_briefly(repo);
        self.signed_in(sign_in, |credential| {
            let handle = self.handle(repo)?.with_stop(stop.clone());
            handle.push_refspec(remote, refspec, track, credential, on_line)
        })
    }

    /// `sign_in` for its own host, else what is stored; kept on success if Remember. A
    /// refusal carries `authRequired` (host, username tried) for the Sign in dialog.
    pub(crate) fn signed_in<T>(
        &self,
        sign_in: Option<&SignIn>,
        run: impl FnOnce(Credentials<'_>) -> Result<T, GitError>,
    ) -> Result<T, GitError> {
        let typed = sign_in.and_then(|sign_in| Some((host_input(&sign_in.host)?, sign_in)));
        // Each host asked, with the username sent there.
        let asked: RefCell<Vec<Asked>> = RefCell::new(Vec::new());
        let used_typed = Cell::new(false);
        let credential = |url: &str| {
            let (host, ssh) = credential_target(url)?;
            let credential = match &typed {
                Some((typed_host, sign_in)) if *typed_host == host && sign_in.is_ssh() == ssh => {
                    used_typed.set(true);
                    Some(sign_in.credential())
                }
                _ => self.stored_credential(url),
            };
            let username = credential.as_ref().and_then(|c| c.username.clone());
            asked.borrow_mut().push(Asked {
                host,
                username,
                ssh,
            });
            credential
        };
        SIGNING_IN.set(true);
        let outcome = run(&credential);
        SIGNING_IN.set(false);
        match outcome {
            Ok(done) => {
                if let Some((_, sign_in)) = typed.filter(|(_, s)| s.remember && used_typed.get())
                    && let Err(err) = self.store_credential(sign_in)
                {
                    tracing::error!(error = ?err, host = %sign_in.host, context = "remembering a sign-in");
                }
                Ok(done)
            }
            Err(GitError::Command(mut failure)) => {
                failure.auth_required = auth_required(&failure, &asked.into_inner());
                Err(GitError::Command(failure))
            }
            Err(other) => Err(other),
        }
    }

    /// Stored credentials only: for what may not ask (a preview, a background probe).
    pub(crate) fn stored(&self) -> impl Fn(&str) -> Option<Credential> + '_ {
        |url| self.stored_credential(url)
    }
}

/// The host git quotes in its refusal (`fetch --all` asks several), else the last one asked;
/// `None` for a host Cogit cannot sign in to (plain http to another machine, ssh).
struct Asked {
    host: String,
    username: Option<String>,
    ssh: bool,
}

fn auth_required(failure: &git_engine::GitCommandError, asked: &[Asked]) -> Option<AuthRequired> {
    let text = format!("{}\n{}", failure.stderr, failure.stdout);
    if !matches!(
        git_engine::credential_trouble(&text),
        Some(CredentialTrouble::NoPrompt | CredentialTrouble::Refused)
    ) {
        return None;
    }
    let (host, ssh) = match git_engine::refused_url(&text) {
        Some(url) => credential_target(&url)?,
        None => asked.last().map(|last| (last.host.clone(), last.ssh))?,
    };
    let username = asked
        .iter()
        .rev()
        .find(|a| a.host == host && a.ssh == ssh)
        .and_then(|a| a.username.clone());
    Some(AuthRequired {
        host,
        username,
        ssh,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_is_answered_only_while_a_sign_in_can_ask() {
        let state = AppState::new();
        let asked = state.signed_in(None, |_| {
            Ok::<_, GitError>(asks_sign_in(Some(CredentialTrouble::NoPrompt)))
        });
        assert!(matches!(asked, Ok(true)));
        assert!(!asks_sign_in(Some(CredentialTrouble::NoPrompt)));
        let other = state.signed_in(None, |_| Ok::<_, GitError>(asks_sign_in(None)));
        assert!(matches!(other, Ok(false)));
    }
}
