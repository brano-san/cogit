//! Hook presets: list, export, remove, install (M10).

use super::{blocking, mutating};
use app_state::{OperationKind, RepoId};
use git_engine::GitError;

#[tauri::command]
#[specta::specta]
pub async fn list_presets(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<app_state::PresetStatus>, GitError> {
    let app_state = state.state.clone();
    blocking("list_presets", move || app_state.presets_for(repo)).await
}

/// Saves the hook as it stands as a preset, so the next repository gets it in one click.
#[tauri::command]
#[specta::specta]
pub async fn export_preset(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    hook: String,
    id: String,
    name: String,
    description: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("export_preset", move || {
        app_state.export_preset(repo, &hook, &id, &name, &description)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_preset(
    state: tauri::State<'_, crate::AppContext>,
    id: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("remove_preset", move || app_state.remove_preset(&id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn install_preset(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    id: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        OperationKind::Other,
        "install_preset",
        move || app_state.install_preset(repo, &id),
    )
    .await
}
