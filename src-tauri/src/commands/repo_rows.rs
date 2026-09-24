//! Rows of the Repositories list that are not on screen, addressed by folder (R-352).

use super::blocking;
use git_engine::{GitError, Submodule};
use std::path::PathBuf;

#[tauri::command]
#[specta::specta]
pub async fn submodule_outline(root: String, parent: String) -> Result<Vec<Submodule>, GitError> {
    blocking("submodule_outline", move || {
        app_state::repo_rows::submodule_outline(&PathBuf::from(root), &parent)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn repo_pulse(root: String) -> Result<git_engine::RepoPulse, GitError> {
    blocking("repo_pulse", move || {
        Ok(app_state::repo_rows::pulse(&PathBuf::from(root)))
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn background_fetch(root: String) -> Result<(), GitError> {
    blocking("background_fetch", move || {
        app_state::repo_rows::background_fetch(&PathBuf::from(root))
    })
    .await
}
