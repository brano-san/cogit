//! The commands behind the graph and Branches context menus (doc/04-ipc-contract.md).

use super::{blocking, mutating};
use app_state::{OperationKind, RepoId};
use git_engine::{FileEntry, GitError, ResetMode};

#[tauri::command]
#[specta::specta]
pub async fn reset_to(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
    mode: ResetMode,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Checkout,
        "reset_to",
        move || app_state.reset_to(repo, &rev, mode),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn is_ancestor(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    ancestor: String,
    descendant: String,
) -> Result<bool, GitError> {
    let app_state = state.state.clone();
    blocking("is_ancestor", move || {
        app_state.is_ancestor(repo, &ancestor, &descendant)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn compare_files(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    from: String,
    to: String,
) -> Result<Vec<FileEntry>, GitError> {
    let app_state = state.state.clone();
    blocking("compare_files", move || {
        app_state.compare_files(repo, &from, &to)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn tag_name_problem(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
) -> Result<Option<String>, GitError> {
    let app_state = state.state.clone();
    blocking("tag_name_problem", move || {
        app_state.tag_name_problem(repo, &name)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn tag_message(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
) -> Result<Option<String>, GitError> {
    let app_state = state.state.clone();
    blocking("tag_message", move || app_state.tag_message(repo, &name)).await
}

#[tauri::command]
#[specta::specta]
pub async fn rename_tag(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    from: String,
    to: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Tag,
        "rename_tag",
        move || app_state.rename_tag(repo, &from, &to),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn rename_stash(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    index: u32,
    message: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Stash,
        "rename_stash",
        move || app_state.rename_stash(repo, index, &message),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn edit_author(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
    name: String,
    email: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Rebase,
        "edit_author",
        move || app_state.edit_author(repo, &rev, &name, &email),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn push_to(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    remote: String,
    refspec: String,
    on_progress: tauri::ipc::Channel<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Push,
        "push_to",
        move || {
            let mut timer = git_engine::phases::PhaseTimer::new();
            let named = remote.clone();
            let result = app_state.push_to(repo, &remote, &refspec, |line| {
                timer.observe(line);
                let _ = on_progress.send(line.to_owned());
            });
            crate::profile::network("push", &named, timer, result.is_ok());
            result
        },
    )
    .await
}
