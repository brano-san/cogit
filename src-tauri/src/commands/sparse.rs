//! Repository ▸ Sparse Checkout (F-955).

use super::blocking;
use app_state::{OperationKind, RepoId};
use git_engine::{GitError, SparseChange, SparseFolder, SparsePreview, SparseState};

#[tauri::command]
#[specta::specta]
pub async fn sparse_state(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<SparseState, GitError> {
    let app_state = state.state.clone();
    blocking("sparse_state", move || app_state.sparse_state(repo)).await
}

/// The folders right inside `dir` in HEAD, read when the dialog opens a folder.
#[tauri::command]
#[specta::specta]
pub async fn sparse_folders(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    dir: String,
) -> Result<Vec<SparseFolder>, GitError> {
    let app_state = state.state.clone();
    blocking("sparse_folders", move || {
        app_state.sparse_folders(repo, &dir)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn sparse_preview(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    change: SparseChange,
) -> Result<SparsePreview, GitError> {
    let app_state = state.state.clone();
    blocking("sparse_preview", move || {
        app_state.sparse_preview(repo, &change)
    })
    .await
}

/// A turn of the lane like a checkout, which the footer can cancel.
#[tauri::command]
#[specta::specta]
pub async fn sparse_apply(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    change: SparseChange,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    super::network::networking_if(
        true,
        &state.state,
        repo,
        OperationKind::Checkout,
        "Sparse checkout",
        "sparse_apply",
        move |stop| app_state.sparse_apply(repo, &change, &stop),
    )
    .await
}
