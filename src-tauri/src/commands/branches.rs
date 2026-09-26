//! Branches and tags from the toolbar and the Branches panel: checkout, create, rename, upstream, delete (M5).

use super::mutating;
use app_state::{OperationKind, RepoId};
use git_engine::{AutostashOutcome, CheckoutTarget, GitError, RemoteDeletion, TagRequest};

#[tauri::command]
#[specta::specta]
pub async fn checkout(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    target: CheckoutTarget,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Checkout,
        "checkout",
        move || app_state.checkout(repo, &target),
    )
    .await
}

/// Stash, check out, apply the stash: one operation of the lane (R-521, R-563).
#[tauri::command]
#[specta::specta]
pub async fn switch_with_autostash(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    target: CheckoutTarget,
    message: String,
    drop_after_clean: bool,
) -> Result<AutostashOutcome, GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Checkout,
        "switch_with_autostash",
        move || app_state.switch_with_autostash(repo, &target, &message, drop_after_clean),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn create_branch(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
    start: Option<String>,
    // Not `switch`: specta puts the parameter name straight into the generated TypeScript,
    // where a reserved word is a syntax error.
    switch_to: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Branch,
        "create_branch",
        move || app_state.create_branch(repo, &name, start.as_deref(), switch_to),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn rename_branch(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    from: String,
    to: String,
    force: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Branch,
        "rename_branch",
        move || app_state.rename_branch(repo, &from, &to, force),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn set_upstream(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    branch: String,
    upstream: Option<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Branch,
        "set_upstream",
        move || app_state.set_upstream(repo, &branch, upstream.as_deref()),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_remote_branch(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    remote: String,
    branch: String,
) -> Result<RemoteDeletion, GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Branch,
        "delete_remote_branch",
        move || app_state.delete_remote_branch(repo, &remote, &branch),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_branch(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
    force: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Branch,
        "delete_branch",
        move || app_state.delete_branch(repo, &name, force),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn create_tag(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    request: TagRequest,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Tag,
        "create_tag",
        move || app_state.create_tag(repo, &request),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_tag(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Tag,
        "delete_tag",
        move || app_state.delete_tag(repo, &name),
    )
    .await
}
