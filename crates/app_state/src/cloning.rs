//! Repository ▸ Clone… (F-575); the frontend opens the result as it opens any folder.

use crate::{AppEvent, AppState, CommandNotice, OperationKind, OperationPermit};
use git_engine::{CloneRequest, GitError, NetworkStop, RemoteBranches};
use std::path::PathBuf;
use std::sync::Arc;

impl AppState {
    pub fn remote_branches(&self, source: &str) -> Result<RemoteBranches, GitError> {
        let token = self.token_for(source.trim());
        git_engine::remote_branches(source, token.as_deref(), Some(&self.unowned_sink()))
    }

    pub async fn enqueue_clone(&self, target: &str) -> OperationPermit<'_> {
        let kind = OperationKind::Clone;
        let lane = PathBuf::from(format!("<clone {}>", target.trim()));
        self.enqueue_detached(lane, kind, kind.title()).await
    }

    pub fn clone_repository(
        &self,
        request: &CloneRequest,
        stop: &NetworkStop,
        on_line: impl FnMut(&str),
    ) -> Result<PathBuf, GitError> {
        let token = self.token_for(request.source.trim());
        let sink = self.unowned_sink();
        git_engine::clone_repository(request, token.as_deref(), stop, Some(&sink), on_line)
    }

    /// Into the journal like any run, with no watcher to quieten: no repository yet.
    fn unowned_sink(&self) -> git_engine::CommandSink {
        let journal = Arc::clone(&self.journal);
        let events = self.events.clone();
        Arc::new(move |entry| {
            let _ = events.send(AppEvent::CommandRecorded(CommandNotice::from(&entry)));
            crate::record(
                &mut journal.write(),
                crate::journal::JOURNAL_CAPACITY,
                entry,
            );
        })
    }
}
