mod commands;
mod logging;

use app_state::AppState;
use specta_typescript::Typescript;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager as _;
use tauri_specta::{Builder, collect_commands};

/// Manifest-relative: `cargo run` starts in the workspace root, not here.
const BINDINGS_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../frontend/src/lib/ipc/bindings.ts"
);

#[derive(Debug)]
pub struct AppContext {
    pub state: Arc<AppState>,
    pub log_path: PathBuf,
}

fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        commands::app_info,
        commands::open_repository,
        commands::load_commits,
        commands::commit_details,
        commands::commit_files
    ])
}

/// A binary rather than a `#[test]`: linking tauri needs the ComCtl32 v6 manifest
/// that `tauri-build` embeds only into binary targets.
pub fn export_bindings() -> anyhow::Result<()> {
    specta_builder()
        .export(Typescript::default(), BINDINGS_PATH)
        .map_err(|err| anyhow::anyhow!("failed to export IPC bindings: {err}"))
}

pub fn run() -> anyhow::Result<()> {
    let specta_builder = specta_builder();

    #[cfg(debug_assertions)]
    if let Err(err) = specta_builder.export(Typescript::default(), BINDINGS_PATH) {
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

            if let Some(window) = app.get_webview_window("main") {
                window.show()?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())?;

    Ok(())
}

#[cfg(debug_assertions)]
fn eprintln_fallback(message: &str) {
    use std::io::Write as _;
    let mut stderr = std::io::stderr();
    let _ = writeln!(stderr, "[cogit] {message}");
}
