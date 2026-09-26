//! The working tree and the index: the file list, staging, discarding, the commit (M6).

use super::{blocking, mutating, mutating_titled};
use app_state::{OperationKind, RepoId};
use diff_engine::PatchRequest;
use git_engine::{CommitRequest, GitError, WorktreeFiles};

#[tauri::command]
#[specta::specta]
pub async fn worktree_files(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    view: git_engine::WorktreeView,
) -> Result<WorktreeFiles, GitError> {
    let app_state = state.state.clone();
    blocking("worktree_files", move || {
        app_state.worktree_files(repo, view)
    })
    .await
}

macro_rules! path_command {
    ($name:ident, $method:ident, $kind:ident) => {
        #[tauri::command]
        #[specta::specta]
        pub async fn $name(
            state: tauri::State<'_, crate::AppContext>,
            repo: RepoId,
            paths: Vec<String>,
        ) -> Result<(), GitError> {
            let app_state = state.state.clone();
            mutating(
                &state.state,
                repo,
                OperationKind::$kind,
                stringify!($name),
                move || app_state.$method(repo, &paths),
            )
            .await
        }
    };
}

path_command!(stage_paths, stage_paths, Stage);

/// Stage all: every change git sees, not a path list (doc/12-risks.md, R-311). `files` is
/// how many rows the list showed, which decides how the blobs are written (R-312).
#[tauri::command]
#[specta::specta]
pub async fn stage_all(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    files: u32,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Stage,
        "stage_all",
        move || app_state.stage_all(repo, files as usize),
    )
    .await
}

path_command!(unstage_paths, unstage_paths, Stage);
path_command!(discard_paths, discard_paths, Discard);
path_command!(add_to_gitignore, add_to_gitignore, Stage);
path_command!(delete_untracked, delete_untracked, Discard);

#[tauri::command]
#[specta::specta]
pub async fn stage_selection(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    request: PatchRequest,
    reverse: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Stage,
        "stage_selection",
        move || app_state.stage_selection(repo, &request, reverse),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn stage_mode(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    executable: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Stage,
        "stage_mode",
        move || app_state.stage_mode(repo, &path, executable),
    )
    .await
}

/// Throws the selected lines away in the working tree. Destructive and journalled; the
/// view is responsible for confirming it first.
#[tauri::command]
#[specta::specta]
pub async fn discard_selection(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    request: PatchRequest,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Discard,
        "discard_selection",
        move || app_state.discard_selection(repo, &request),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn commit(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    request: CommitRequest,
) -> Result<String, GitError> {
    let app_state = state.state.clone();
    let oid = mutating(
        &state.state,
        repo,
        OperationKind::Commit,
        "commit",
        move || app_state.commit(repo, &request),
    )
    .await?;

    tracing::info!(repo = repo.0, oid = %oid, "commit created");
    if state.state.wants_maintenance(repo).unwrap_or(false) {
        // The commit's caller does not wait for it; the queue does (R-444).
        let app_state = state.state.clone();
        tauri::async_runtime::spawn(async move {
            let ran = app_state.clone();
            let maintained = mutating_titled(
                &app_state,
                repo,
                OperationKind::Other,
                "Maintaining the repository",
                "maintenance",
                move || ran.maintain_after_commit(repo),
            )
            .await;
            if let Err(err) = maintained {
                tracing::error!(error = ?err, context = "auto maintenance after a commit");
            }
        });
    }
    Ok(oid)
}
