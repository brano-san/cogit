//! Repositories and submodules: open, scan, list, close; their refs, config and health.

use super::{blocking, mutating};
use app_state::{OperationKind, RepoId, RepoOverview, RepoSummary};
use git_engine::{GitError, Submodule};
use serde::Serialize;
use std::path::PathBuf;

#[tauri::command]
#[specta::specta]
pub async fn open_repository(
    state: tauri::State<'_, crate::AppContext>,
    path: String,
) -> Result<RepoSummary, GitError> {
    let app_state = state.state.clone();
    let path = PathBuf::from(path);

    let started = std::time::Instant::now();
    let summary = blocking("open_repository", move || app_state.open_repository(&path)).await?;

    tracing::info!(
        repo = summary.repo.0,
        name = %summary.name,
        path = %summary.root,
        branches = summary.branches.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "repository opened"
    );
    Ok(summary)
}

/// The panels' re-read of the repository they show: by id, so it never lists a submodule
/// or worktree opened from a tree (R-543).
#[tauri::command]
#[specta::specta]
pub async fn reread_repository(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<RepoSummary, GitError> {
    let app_state = state.state.clone();
    blocking("reread_repository", move || {
        app_state.reread_repository(repo)
    })
    .await
}

/// What travels up the channel while a folder scan runs; `Started` carries the id
/// `cancel_operation` takes.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ScanChunk {
    Started { id: u32 },
    Found { hit: app_state::ScanHit },
}

/// A folder can hold hundreds of repositories, so hits stream in as they are found.
/// The walk ends on `cancel_operation`: a channel the page stopped listening to still
/// accepts every send.
#[tauri::command]
#[specta::specta]
pub async fn scan_for_repositories(
    state: tauri::State<'_, crate::AppContext>,
    cancellations: tauri::State<'_, std::sync::Arc<crate::operations::Cancellations>>,
    path: String,
    max_depth: u32,
    on_found: tauri::ipc::Channel<ScanChunk>,
) -> Result<u32, GitError> {
    let app_state = state.state.clone();
    let path = PathBuf::from(path);
    let depth = max_depth.clamp(1, 12) as usize;
    let cancellations = std::sync::Arc::clone(&cancellations);
    let (id, cancel) = cancellations.start();
    let _ = on_found.send(ScanChunk::Started { id });

    let found = blocking("scan_for_repositories", move || {
        let mut found = 0_u32;
        app_state.scan_for_repositories(
            &path,
            depth,
            || cancel.is_cancelled(),
            |hit| {
                found += 1;
                on_found.send(ScanChunk::Found { hit }).is_ok()
            },
        );
        Ok(found)
    })
    .await;
    cancellations.finish(id);
    found
}

/// Each row not cached costs a `head`, a `branches` and a `status`, so it leaves the
/// async workers that carry IPC.
#[tauri::command]
#[specta::specta]
pub async fn repositories(
    state: tauri::State<'_, crate::AppContext>,
) -> Result<Vec<RepoOverview>, GitError> {
    let app_state = state.state.clone();
    blocking("repositories", move || Ok(app_state.overviews())).await
}

/// Answers with the repositories left open, which the caller would otherwise ask for next.
#[tauri::command]
#[specta::specta]
pub async fn close_repository(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<RepoOverview>, GitError> {
    let app_state = state.state.clone();
    // Off the main thread: stopping a watcher joins the thread that delivers its events,
    // and a join on the message loop is a frozen window (doc/12-risks.md, R-126).
    blocking("close_repository", move || {
        app_state.close_repository(repo);
        Ok(app_state.overviews())
    })
    .await
}

/// The repository the panels show, the only one watched (R-351). Off the main thread for
/// the same join as `close_repository`.
#[tauri::command]
#[specta::specta]
pub async fn show_repository(
    state: tauri::State<'_, crate::AppContext>,
    repo: Option<RepoId>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("show_repository", move || {
        app_state.show_repository(repo);
        Ok(())
    })
    .await
}

/// Refs and state without reopening the repository, for the refresh after a commit.
#[tauri::command]
#[specta::specta]
pub async fn repo_refs(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<app_state::RepoRefs, GitError> {
    let app_state = state.state.clone();
    blocking("repo_refs", move || app_state.repo_refs(repo)).await
}

/// The counters and the conflicted paths from one read, for the refresh after a mutation.
#[tauri::command]
#[specta::specta]
pub async fn working_state(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<git_engine::WorkingState, GitError> {
    let app_state = state.state.clone();
    blocking("working_state", move || app_state.working_state(repo)).await
}

/// Opens a submodule from its node in the tree: the panels follow it, the Repositories
/// panel does not gain an entry for it (doc/12-risks.md, R-109).
#[tauri::command(async)]
#[specta::specta]
pub async fn open_submodule(
    state: tauri::State<'_, crate::AppContext>,
    owner: RepoId,
    key: String,
) -> Result<RepoSummary, GitError> {
    let app_state = state.state.clone();
    blocking("open_submodule", move || {
        app_state.open_submodule(owner, &key)
    })
    .await
}

/// The submodules directly under `parent`; empty `parent` means the top level.
#[tauri::command]
#[specta::specta]
pub async fn list_submodules(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    parent: String,
) -> Result<Vec<Submodule>, GitError> {
    let app_state = state.state.clone();
    let found = blocking("list_submodules", move || {
        app_state.submodules_under(repo, &parent)
    })
    .await?;
    tracing::debug!(repo = repo.0, submodules = found.len(), "submodules listed");
    Ok(found)
}

#[tauri::command]
#[specta::specta]
pub async fn update_submodule(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    init: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Submodule,
        "update_submodule",
        move || app_state.update_submodule(repo, &path, init),
    )
    .await
}

/// Repository ▸ Edit Git Config. `repo` is only read for the repository scope.
#[tauri::command]
#[specta::specta]
pub async fn read_git_config(
    state: tauri::State<'_, crate::AppContext>,
    repo: Option<RepoId>,
    scope: git_engine::ConfigScope,
) -> Result<git_engine::ConfigFile, GitError> {
    let app_state = state.state.clone();
    blocking("read_git_config", move || {
        app_state.config_file(repo, scope)
    })
    .await
}

/// Written only after `git config --file` has read the text back without complaint.
#[tauri::command]
#[specta::specta]
pub async fn write_git_config(
    state: tauri::State<'_, crate::AppContext>,
    repo: Option<RepoId>,
    scope: git_engine::ConfigScope,
    text: String,
    crlf: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    let save = {
        let app_state = app_state.clone();
        move || app_state.save_config_file(repo, scope, &text, crlf)
    };
    match repo {
        Some(id) => {
            mutating(
                &app_state,
                id,
                OperationKind::Other,
                "write_git_config",
                save,
            )
            .await
        }
        None => blocking("write_git_config", save).await,
    }
}

/// Run in the background after a repository opens; nothing in it changes the repository.
#[tauri::command]
#[specta::specta]
pub async fn repository_health(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<git_engine::HealthFinding>, GitError> {
    let app_state = state.state.clone();
    blocking("repository_health", move || app_state.health(repo)).await
}
