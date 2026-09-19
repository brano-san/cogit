//! Tauri IPC commands.
//!
//! Command bodies stay thin (INV-09): fetch state, call a crate, map the error.
//! Any logic beyond that belongs in `crates/*`.
//!
//! Every command must also be listed in `doc/04-ipc-contract.md` section 4.

use serde::Serialize;

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
