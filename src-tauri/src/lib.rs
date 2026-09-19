//! Application shell: IPC routing, windows, plugins, state injection.
//!
//! No Git logic lives here (INV-09). Command bodies delegate to `crates/*`.

mod commands;
mod logging;

use app_state::AppState;
use specta_typescript::Typescript;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager as _;
use tauri_specta::{Builder, collect_commands};

/// Where generated bindings land.
///
/// Resolved from `CARGO_MANIFEST_DIR` at compile time rather than from the working
/// directory: `cargo run` starts in the workspace root, so a plain relative path would
/// write the file outside the repository.
///
/// Regenerated on every debug run; never edited by hand (INV-10).
const BINDINGS_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../frontend/src/lib/ipc/bindings.ts"
);

/// Everything the commands need, managed by Tauri and shared across windows.
#[derive(Debug)]
pub struct AppContext {
    pub state: Arc<AppState>,
    pub log_path: PathBuf,
}

/// The single registry of IPC commands and events.
fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        commands::app_info,
        commands::open_repository,
        commands::load_commits
    ])
}

/// Writes `bindings.ts` from the command registry.
///
/// Exposed so `cargo run -p cogit --bin export-bindings` can refresh the frontend types
/// without launching the application — the pre-commit hook relies on this.
///
/// This lives in a binary rather than a `#[test]` on purpose: linking `tauri` pulls in
/// ComCtl32 v6, whose side-by-side manifest `tauri-build` only embeds into **binary**
/// targets. A test harness without that manifest fails to start with
/// `STATUS_ENTRYPOINT_NOT_FOUND` before a single test runs.
///
/// # Errors
/// Returns an error if the bindings file cannot be written.
pub fn export_bindings() -> anyhow::Result<()> {
    specta_builder()
        .export(Typescript::default(), BINDINGS_PATH)
        .map_err(|err| anyhow::anyhow!("failed to export IPC bindings: {err}"))
}

/// Builds and runs the application.
///
/// # Errors
/// Returns an error if logging cannot be initialised or the Tauri runtime fails to start.
pub fn run() -> anyhow::Result<()> {
    let specta_builder = specta_builder();

    // Exporting on every debug run keeps the frontend types in lockstep with Rust.
    // The pre-commit hook fails if the result differs from what is committed.
    #[cfg(debug_assertions)]
    if let Err(err) = specta_builder.export(Typescript::default(), BINDINGS_PATH) {
        // Not fatal: a developer running the app should not be blocked by a write failure.
        eprintln_fallback(&format!("failed to export IPC bindings: {err}"));
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app| {
            let log_dir = app.path().app_log_dir()?;
            // The guard must outlive the process; it is owned by AppContext below.
            let guard = logging::init(&log_dir)?;

            tracing::info!(
                version = env!("CARGO_PKG_VERSION"),
                log_dir = %log_dir.display(),
                "cogit starting"
            );

            app.manage(AppContext {
                state: Arc::new(AppState::new()),
                log_path: log_dir.join("cogit.log"),
            });
            app.manage(guard);

            specta_builder.mount_events(app);

            // The window starts hidden so the user never sees an unstyled white flash.
            if let Some(window) = app.get_webview_window("main") {
                window.show()?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())?;

    Ok(())
}

/// Reports a failure that happens before `tracing` is available.
///
/// Logging is initialised inside `setup`, so the binding export above has no subscriber
/// yet. This is the one place a direct write to stderr is justified.
#[cfg(debug_assertions)]
fn eprintln_fallback(message: &str) {
    use std::io::Write as _;
    let mut stderr = std::io::stderr();
    let _ = writeln!(stderr, "[cogit] {message}");
}
