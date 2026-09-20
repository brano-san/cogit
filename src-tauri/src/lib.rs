mod commands;
mod logging;
mod menu;
mod profile;

use app_state::AppState;
use specta_typescript::Typescript;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager as _;
use tauri_specta::{Builder, Event as _, collect_commands, collect_events};

/// Manifest-relative: `cargo run` starts in the workspace root, not here.
const BINDINGS_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../frontend/src/lib/ipc/bindings.ts"
);

/// Mirrors `app_state::AppEvent::RepoChanged`. It lives here because deriving
/// `tauri_specta::Event` would make `app_state` depend on tauri (INV-09).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct RepoChanged {
    pub repo: app_state::RepoId,
    pub kind: fs_watcher::ChangeKind,
}

/// A native menu item was chosen. The payload is the palette command id, so the frontend
/// runs the same code path the palette would.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
pub struct MenuCommand(pub String);

/// Mirrors `app_state::AppEvent::AvatarReady`: one row can redraw without a refetch.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct AvatarReady {
    pub email: String,
}

/// Mirrors `app_state::AppEvent::Operation*`, for the spinner in the toolbar.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct OperationChanged {
    pub id: u32,
    pub label: String,
    /// `None` while it runs; `Some` once it is over.
    pub success: Option<bool>,
}

#[derive(Debug)]
pub struct AppContext {
    pub state: Arc<AppState>,
    pub log_path: PathBuf,
}

fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .events(collect_events![
            RepoChanged,
            MenuCommand,
            OperationChanged,
            AvatarReady
        ])
        .commands(collect_commands![
            commands::app_info,
            commands::open_repository,
            commands::load_commits,
            commands::commit_details,
            commands::commit_files,
            commands::diff_file,
            commands::worktree_files,
            commands::repo_status,
            commands::stage_paths,
            commands::unstage_paths,
            commands::discard_paths,
            commands::commit,
            commands::checkout,
            commands::create_branch,
            commands::delete_branch,
            commands::command_log,
            commands::command_problems,
            commands::clear_command_log,
            commands::safety_log,
            commands::undo_last,
            commands::abort_operation,
            commands::continue_operation,
            commands::stashes,
            commands::stash_push,
            commands::stash_apply,
            commands::stash_drop,
            commands::create_tag,
            commands::delete_tag,
            commands::remotes,
            commands::fetch,
            commands::pull,
            commands::push,
            commands::merge,
            commands::rebase,
            commands::skip_operation,
            commands::cherry_pick,
            commands::revert,
            commands::reflog,
            commands::lost_commits,
            commands::repositories,
            commands::close_repository,
            commands::submodules,
            commands::update_submodule,
            commands::stage_selection,
            commands::blame,
            commands::remote_url,
            commands::add_to_gitignore,
            commands::delete_untracked,
            commands::image_sides,
            commands::conflicted_paths,
            commands::conflict_text,
            commands::resolve_conflict,
            commands::resolve_conflict_text,
            commands::find_object,
            commands::rename_branch,
            commands::set_upstream,
            commands::delete_remote_branch,
            commands::undo_entry,
            commands::stash_contents,
            commands::stash_selection,
            commands::run_check,
            commands::worktrees,
            commands::worktree_holding,
            commands::add_worktree,
            commands::remove_worktree,
            commands::prune_worktrees,
            commands::avatars,
            commands::set_avatars,
            commands::terminal_choices,
            commands::open_in_terminal,
            commands::set_menu_state,
            // ─── diff-merge branch appends below; master inserts above ───
            commands::report_timing,
            commands::default_keymap,
            commands::set_keymap,
            commands::scan_for_repositories,
            commands::has_token,
            commands::store_token,
            commands::forget_token,
            commands::list_hooks,
            commands::read_hook,
            commands::write_hook,
            commands::set_hook_enabled,
            commands::use_hooks_path,
            commands::run_hook,
            commands::rollback_to,
            commands::is_published,
            commands::split_off,
            commands::rebase_todo,
            commands::interactive_rebase,
            commands::rebase_progress,
            commands::overlap_window,
            commands::bypass_log,
            commands::popup_context_menu,
            commands::open_compare_window,
            commands::commit_template,
            commands::stage_mode,
            commands::list_presets,
            commands::install_preset,
            commands::diff_files,
            commands::file_before
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
            let config_dir = app.path().app_config_dir()?;
            let guard = logging::init(&log_dir, &config_dir)?;

            tracing::info!(
                version = env!("CARGO_PKG_VERSION"),
                log_dir = %log_dir.display(),
                "cogit starting"
            );

            let state = Arc::new(AppState::new());
            app.manage(AppContext {
                state: Arc::clone(&state),
                log_path: log_dir.join("cogit.log"),
            });
            app.manage(guard);

            specta_builder.mount_events(app);
            forward_repo_changes(app.handle().clone(), &state);

            app.manage(menu::ContextMenu::<tauri::Wry>::default());
            let stored = menu::stored_keymap(&config_dir);
            let (menu, collected) = menu::build(app.handle(), &stored)?;
            app.set_menu(menu)?;
            app.manage(menu::MenuItems::from(collected));
            let keymap = menu::Keymap::default();
            keymap.set(stored);
            app.manage(keymap);
            app.on_menu_event(|app, event| {
                let _ = MenuCommand(event.id().0.clone()).emit(app);
            });

            if let Some(window) = app.get_webview_window("main") {
                window.show()?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())?;

    Ok(())
}

/// The watcher runs on its own thread, so events cross into the webview here.
fn forward_repo_changes(app: tauri::AppHandle, state: &Arc<AppState>) {
    let mut events = state.subscribe();
    tauri::async_runtime::spawn(async move {
        while let Ok(event) = events.recv().await {
            match event {
                app_state::AppEvent::RepoChanged { repo, kind } => {
                    let _ = RepoChanged { repo, kind }.emit(&app);
                }
                app_state::AppEvent::AvatarReady { email } => {
                    let _ = AvatarReady { email }.emit(&app);
                }
                app_state::AppEvent::OperationStarted { id, label } => {
                    let _ = OperationChanged {
                        id,
                        label,
                        success: None,
                    }
                    .emit(&app);
                }
                app_state::AppEvent::OperationFinished { id, success } => {
                    let _ = OperationChanged {
                        id,
                        label: String::new(),
                        success: Some(success),
                    }
                    .emit(&app);
                }
                _ => {}
            }
        }
    });
}

#[cfg(debug_assertions)]
fn eprintln_fallback(message: &str) {
    use std::io::Write as _;
    let mut stderr = std::io::stderr();
    let _ = writeln!(stderr, "[cogit] {message}");
}
