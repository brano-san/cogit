//! Merge, rebase, cherry-pick, revert, the operation in progress, commit surgery (M12).

use super::{blocking, mutating, mutating_titled};
use app_state::{OperationKind, RepoId};
use git_engine::{GitError, MergeOptions, RebaseOptions};

macro_rules! repo_command {
    ($name:ident, $kind:ident, $title:literal) => {
        #[tauri::command]
        #[specta::specta]
        pub async fn $name(
            state: tauri::State<'_, crate::AppContext>,
            repo: RepoId,
        ) -> Result<(), GitError> {
            let app_state = state.state.clone();
            mutating_titled(
                &state.state,
                repo,
                OperationKind::$kind,
                $title,
                stringify!($name),
                move || app_state.$name(repo),
            )
            .await
        }
    };
}

// Merge, rebase, cherry-pick, revert or `git am`: the footer says what is done to it.
repo_command!(abort_operation, Merge, "Aborting");
repo_command!(continue_operation, Merge, "Continuing");
repo_command!(skip_operation, Merge, "Skipping");

#[tauri::command]
#[specta::specta]
pub async fn merge(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    options: MergeOptions,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Merge,
        "merge",
        move || app_state.merge(repo, &options),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn rebase(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    options: RebaseOptions,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Rebase,
        "rebase",
        move || app_state.rebase(repo, &options),
    )
    .await
}

macro_rules! replay_command {
    ($name:ident, $title:literal) => {
        #[tauri::command]
        #[specta::specta]
        pub async fn $name(
            state: tauri::State<'_, crate::AppContext>,
            repo: RepoId,
            commits: Vec<String>,
        ) -> Result<(), GitError> {
            let app_state = state.state.clone();
            mutating_titled(
                &state.state,
                repo,
                OperationKind::Commit,
                $title,
                stringify!($name),
                move || app_state.$name(repo, &commits),
            )
            .await
        }
    };
}

replay_command!(cherry_pick, "Cherry-picking");
replay_command!(revert, "Reverting");

#[tauri::command]
#[specta::specta]
pub async fn rebase_todo(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    base: String,
) -> Result<Vec<git_engine::TodoEntry>, GitError> {
    let app_state = state.state.clone();
    blocking("rebase_todo", move || app_state.rebase_todo(repo, &base)).await
}

#[tauri::command]
#[specta::specta]
pub async fn interactive_rebase(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    base: String,
    plan: Vec<git_engine::TodoEntry>,
    paused: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Rebase,
        "interactive_rebase",
        move || app_state.interactive_rebase(repo, &base, &plan, paused),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn rebase_progress(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Option<git_engine::RebaseProgress>, GitError> {
    let app_state = state.state.clone();
    blocking("rebase_progress", move || app_state.rebase_progress(repo)).await
}

#[tauri::command]
#[specta::specta]
pub async fn rollback_to(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
    paths: Vec<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating_titled(
        &state.state,
        repo,
        OperationKind::Undo,
        "Rolling back",
        "rollback_to",
        move || app_state.rollback_to(repo, &rev, &paths),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn split_off(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
    paths: Vec<String>,
    message: String,
    split_first: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating_titled(
        &state.state,
        repo,
        OperationKind::Commit,
        "Splitting",
        "split_off",
        move || app_state.split_off(repo, &rev, &paths, &message, split_first),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn is_published(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
) -> Result<bool, GitError> {
    let app_state = state.state.clone();
    blocking("is_published", move || app_state.is_published(repo, &rev)).await
}

/// The shared branches that already hold this commit; empty means it is safe to rewrite.
#[tauri::command]
#[specta::specta]
pub async fn protecting_refs(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
) -> Result<Vec<String>, GitError> {
    let app_state = state.state.clone();
    blocking("protecting_refs", move || {
        app_state.protecting_refs(repo, &rev)
    })
    .await
}
