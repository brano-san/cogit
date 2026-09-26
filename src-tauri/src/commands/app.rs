//! The application rather than a repository: About, licences, settings, the webview's reports.

use super::{blocking, blocking_or_default};
use git_engine::GitError;
use serde::{Deserialize, Serialize};

/// specta follows serde, so a DTO without `camelCase` reads `undefined` in the UI.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub debug_build: bool,
    /// What a bug report needs to identify the build (doc/12-risks.md, R-146).
    pub commit: String,
    /// Built from a tree with uncommitted code, so the commit alone does not describe it.
    pub dirty: bool,
    #[specta(type = specta_typescript::Number)]
    pub built_at: i64,
    pub repository: String,
    pub os: app_state::environment::OsInfo,
    /// The engine actually rendering this window — the one the user has to update when a
    /// CSS feature is missing, and the one Cogit cannot ship itself.
    pub renderer: String,
    pub git: String,
    pub rustc: String,
    pub tauri: String,
    /// The `gix` version the reads go through.
    pub git_library: String,
    pub log_path: String,
    pub log_dir: String,
    pub settings_path: String,
    pub displays: Vec<app_state::environment::DisplayInfo>,
}

/// The Rust half of the third-party licence list, written by `build.rs`.
const THIRD_PARTY_CRATES: &str = include_str!(concat!(env!("OUT_DIR"), "/third-party-crates.txt"));

/// Async: it spawns `git --version` and reads the registry, neither of which belongs on
/// the main thread that a plain command runs on.
#[tauri::command]
#[specta::specta]
pub async fn app_info(
    window: tauri::Window,
    state: tauri::State<'_, crate::AppContext>,
) -> Result<AppInfo, GitError> {
    let log_path = state.log_path.clone();
    let settings_path = app_state::settings::path(&state.config_dir);
    let displays = displays(&window);

    blocking("app_info", move || {
        let webview = tauri::webview_version().ok();
        Ok(AppInfo {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            debug_build: cfg!(debug_assertions),
            commit: env!("COGIT_COMMIT").to_owned(),
            dirty: env!("COGIT_DIRTY") == "true",
            built_at: env!("COGIT_BUILT_AT").parse().unwrap_or_default(),
            repository: env!("CARGO_PKG_REPOSITORY").to_owned(),
            os: app_state::environment::os_info(),
            renderer: app_state::environment::renderer_label(
                std::env::consts::OS,
                webview.as_deref(),
            ),
            git: git_engine::git_version().unwrap_or_else(|_| "not found".to_owned()),
            rustc: env!("COGIT_RUSTC").to_owned(),
            tauri: tauri::VERSION.to_owned(),
            git_library: git_engine::gix_version().to_owned(),
            log_dir: log_path
                .parent()
                .map(|dir| dir.display().to_string())
                .unwrap_or_default(),
            log_path: log_path.display().to_string(),
            settings_path: settings_path.display().to_string(),
            displays,
        })
    })
    .await
}

/// The same monitor list `window_place` reads at start-up; read-only.
fn displays(window: &tauri::Window) -> Vec<app_state::environment::DisplayInfo> {
    let monitors = window.available_monitors().unwrap_or_else(|err| {
        tracing::warn!(error = ?err, context = "cannot list the monitors for About");
        Vec::new()
    });
    let primary = window.primary_monitor().ok().flatten();
    monitors
        .iter()
        .map(|monitor| app_state::environment::DisplayInfo {
            name: monitor.name().cloned(),
            width: monitor.size().width,
            height: monitor.size().height,
            scale: monitor.scale_factor(),
            primary: primary
                .as_ref()
                .is_some_and(|main| main.position() == monitor.position()),
        })
        .collect()
}

/// Help ▸ About ▸ Third-party licences. `frontend` is the list the Vite build shipped
/// beside the page; the dev server has none.
#[tauri::command]
#[specta::specta]
pub async fn open_third_party_licences(
    app: tauri::AppHandle,
    frontend: Option<String>,
) -> Result<(), GitError> {
    let text = app_state::licences::document(
        env!("CARGO_PKG_VERSION"),
        THIRD_PARTY_CRATES,
        frontend.as_deref(),
    );
    let path = blocking("open_third_party_licences", move || {
        app_state::licences::write(&std::env::temp_dir(), &text)
            .map_err(|err| GitError::Io(format!("cannot write the licence list: {err}")))
    })
    .await?;
    tauri_plugin_opener::OpenerExt::opener(&app)
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(|err| GitError::Io(format!("cannot open {}: {err}", path.display())))
}

/// The settings document as JSON text. Rust owns the file because the menu and the
/// logger read it before there is a window to ask.
#[tauri::command]
#[specta::specta]
pub async fn read_settings(app: tauri::AppHandle) -> String {
    let dir = tauri::Manager::state::<crate::AppContext>(&app)
        .config_dir
        .clone();
    blocking_or_default("read_settings", move || {
        app_state::settings::read_document(&dir).to_string()
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn write_setting(
    state: tauri::State<'_, crate::AppContext>,
    key: String,
    value: String,
) -> Result<(), GitError> {
    // Parsed here rather than stored raw: the same file is read back by the logger and
    // the menu, and a malformed value would take both down with it.
    let parsed = serde_json::from_str(&value)
        .map_err(|err| GitError::Internal(format!("settings value is not JSON: {err}")))?;
    let dir = state.config_dir.clone();
    blocking("write_setting", move || {
        app_state::settings::write_key(&dir, &key, parsed)
            .map_err(|err| GitError::Io(format!("cannot write the settings: {err}")))
    })
    .await
}

/// The webview's own clock: how long the user waited between an action and the screen
/// showing its result. Only the backend half is visible from Rust.
#[tauri::command]
#[specta::specta]
pub fn report_timing(label: String, ms: u32, detail: String) {
    crate::profile::ui(&label, u64::from(ms), &detail);
}

/// One line of the webview's log. `message` starts with the webview's own `+Nms`: a
/// batch lands at once, so the file's timestamp is when it arrived, not when it was said.
#[derive(Debug, Deserialize, specta::Type)]
pub struct WebviewLogLine {
    pub level: String,
    pub message: String,
    pub context: String,
}

/// The webview's own log lines, into the same file, in batches.
///
/// A JS error that only reaches the devtools console dies with the renderer — which is
/// exactly the moment it was worth keeping.
#[tauri::command]
#[specta::specta]
pub fn log_from_frontend(lines: Vec<WebviewLogLine>) {
    for WebviewLogLine {
        level,
        message,
        context,
    } in lines
    {
        match level.as_str() {
            "error" => tracing::error!(target: "cogit::webview", context, "{message}"),
            "warn" => tracing::warn!(target: "cogit::webview", context, "{message}"),
            "debug" => tracing::debug!(target: "cogit::webview", context, "{message}"),
            _ => tracing::info!(target: "cogit::webview", context, "{message}"),
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn report_memory(sample: crate::profile::RendererMemory) {
    crate::profile::renderer(&sample);
}

/// Answered by the page to show it is still running while a close is pending.
///
/// Injected by `shutdown::watch`, not called from `frontend/`: the whole point is that
/// it works without the page knowing about it (problem 13).
#[tauri::command]
#[specta::specta]
pub fn closing_ping() {
    crate::shutdown::answered();
}
