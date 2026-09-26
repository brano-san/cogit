//! Stashes: list, save, apply, drop, and a selection or contents of one (M5).

use super::{blocking, mutating};
use app_state::{OperationKind, RepoId};
use git_engine::{GitError, StashEntry, StashOptions};

#[tauri::command]
#[specta::specta]
pub async fn stash_selection(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    paths: Vec<String>,
    message: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Stash,
        "stash_selection",
        move || app_state.stash_selection(repo, &paths, &message),
    )
    .await
}

/// The three parts of a stash, read without applying it (T5.2).
#[tauri::command]
#[specta::specta]
pub async fn stash_contents(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    index: u32,
) -> Result<git_engine::StashContents, GitError> {
    let app_state = state.state.clone();
    blocking("stash_contents", move || {
        app_state.stash_contents(repo, index)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn stashes(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<StashEntry>, GitError> {
    let app_state = state.state.clone();
    blocking("stashes", move || app_state.stashes(repo)).await
}

#[tauri::command]
#[specta::specta]
pub async fn stash_push(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    options: StashOptions,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Stash,
        "stash_push",
        move || app_state.stash_push(repo, &options),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn stash_apply(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    index: u32,
    pop: bool,
    restore_index: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Stash,
        "stash_apply",
        move || app_state.stash_apply(repo, index, pop, restore_index),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn stash_drop(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    index: u32,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Stash,
        "stash_drop",
        move || app_state.stash_drop(repo, index),
    )
    .await
}
