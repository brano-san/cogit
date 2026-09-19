//! Tauri IPC commands.
//!
//! Command bodies stay thin (INV-09): fetch state, call a crate, map the error.
//! Any logic beyond that belongs in `crates/*`.
//!
//! Every command must also be listed in `doc/04-ipc-contract.md` section 4.

use app_state::RepoSummary;
use git_engine::GitError;
use serde::Serialize;
use std::path::PathBuf;

/// Static facts about the running application, shown in the status bar and in bug reports.
///
/// `rename_all = "camelCase"` is mandatory on every DTO: specta follows serde for field
/// names, and mixing `snake_case` and `camelCase` across the IPC boundary is a constant
/// source of silent `undefined` reads in the UI.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    /// Absolute path of the current log file, so the user can open it from the UI.
    pub log_path: String,
    /// Whether this is a debug build. The UI shows developer affordances only then.
    pub debug_build: bool,
}

/// Returns application metadata.
///
/// This is the walking-skeleton command: it proves the whole
/// Rust → specta → TypeScript → Svelte chain is wired correctly.
#[tauri::command]
#[specta::specta]
pub fn app_info(state: tauri::State<'_, crate::AppContext>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        log_path: state.log_path.display().to_string(),
        debug_build: cfg!(debug_assertions),
    }
}

/// Opens the repository containing `path` and returns what the UI needs to render it.
///
/// # Errors
/// Returns [`GitError`] if the path encloses no repository or its refs cannot be read.
#[tauri::command]
#[specta::specta]
pub async fn open_repository(
    state: tauri::State<'_, crate::AppContext>,
    path: String,
) -> Result<RepoSummary, GitError> {
    // `gix` reads the ref store synchronously, so this belongs off the async runtime
    // (doc/01-architecture.md section 3). The Arc is cloned first because the Tauri
    // state guard cannot be held across the await.
    let app_state = state.state.clone();
    let path = PathBuf::from(path);

    let started = std::time::Instant::now();
    let summary = tokio::task::spawn_blocking(move || app_state.open_repository(&path))
        .await
        .map_err(|err| GitError::Internal(format!("open_repository task failed: {err}")))??;

    tracing::info!(
        repo = summary.repo.0,
        name = %summary.name,
        branches = summary.branches.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "repository opened"
    );
    Ok(summary)
}
