use app_state::{OperationKind, RepoId};
use git_engine::GitError;

pub mod avatars;
pub mod bisect;
pub mod branches;
pub mod clone;
pub mod conflicts;
pub mod desktop;
pub mod file_ops;
pub mod flow;
pub mod hooks;
pub mod investigate;
pub mod network;
pub mod presets;
pub mod ref_ops;
pub mod remote_ops;
pub mod remotes;
pub mod repo_rows;
pub mod stash;
pub mod toolbar;
pub mod worktrees;

pub mod app;
pub mod commit_graph;
pub mod contents;
pub mod file_history;
pub mod journal;
pub mod repository;
pub mod rewrite;
pub mod staging;
pub mod terminal;
pub mod windows;

// Globbed, so these commands keep their `commands::<name>` path and the hidden macros
// that `collect_commands!` looks up beside each one.
pub use app::*;
pub use commit_graph::*;
pub use contents::*;
pub use file_history::*;
pub use journal::*;
pub use repository::*;
pub use rewrite::*;
pub use staging::*;
pub use terminal::*;
pub use windows::*;

/// Every blocking command goes through here, so the profile log holds one line per IPC
/// call: what ran, how long it took and whether it worked.
async fn blocking<T, F>(label: &'static str, work: F) -> Result<T, GitError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, GitError> + Send + 'static,
{
    let started = std::time::Instant::now();
    let joined = tokio::task::spawn_blocking(work)
        .await
        .map_err(|err| GitError::Internal(format!("{label} task failed: {err}")))?;
    crate::profile::call(label, started.elapsed(), joined.is_ok());
    joined
}

/// `blocking` for a command whose answer is not a `Result`: a task that failed to run is
/// logged and answers the default.
async fn blocking_or_default<T, F>(label: &'static str, work: F) -> T
where
    T: Default + Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    blocking(label, move || Ok(work()))
        .await
        .unwrap_or_else(|err| {
            tracing::error!(error = ?err, context = label);
            T::default()
        })
}

/// A mutation waits for its turn in the repository's lane before it starts (P1.3).
///
/// Three clicks on push are three pushes, one after the other, in the order they landed —
/// not three `git push` processes racing for the same ref lock, and not two of them
/// silently dropped.
async fn mutating<T, F>(
    state: &std::sync::Arc<app_state::AppState>,
    repo: RepoId,
    kind: OperationKind,
    label: &'static str,
    work: F,
) -> Result<T, GitError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, GitError> + Send + 'static,
{
    mutating_titled(state, repo, kind, kind.title(), label, work).await
}

/// `mutating` with a footer line of its own, for work the kind's title says too little about.
async fn mutating_titled<T, F>(
    state: &std::sync::Arc<app_state::AppState>,
    repo: RepoId,
    kind: OperationKind,
    title: &str,
    label: &'static str,
    work: F,
) -> Result<T, GitError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, GitError> + Send + 'static,
{
    let permit = state.enqueue(repo, kind, title).await;
    let result = blocking(label, work).await;
    permit.finish(result.is_ok());
    result
}

#[cfg(test)]
mod tests;
