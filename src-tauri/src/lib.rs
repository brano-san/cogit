mod accelerators;
mod blame_window;
mod child_window;
mod commands;
mod diagnostics;
mod events;
mod logging;
mod menu;
mod operations;
mod profile;
mod recycle_bin;
#[cfg(windows)]
mod renderer_failure;
#[cfg(windows)]
mod session_end;
mod shutdown;
#[cfg(windows)]
mod webview2;
mod webview_memory;
mod window_place;

use app_state::AppState;
pub use events::{
    AvatarReady, CommandRecorded, MenuCommand, MergeResolved, OperationChanged, RepoChanged,
    SessionEnding,
};
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

#[derive(Debug)]
pub struct AppContext {
    pub state: Arc<AppState>,
    pub log_path: PathBuf,
    pub config_dir: PathBuf,
}

fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        // What `graph_window` bytes decode to (R-194): no command returns it as JSON any more.
        .typ::<graph_engine::GraphRow>()
        .events(collect_events![
            RepoChanged,
            MenuCommand,
            OperationChanged,
            AvatarReady,
            MergeResolved,
            CommandRecorded,
            SessionEnding
        ])
        .commands(collect_commands![
            commands::app_info,
            commands::open_third_party_licences,
            commands::open_repository,
            commands::load_commits,
            commands::graph_window,
            commands::graph_row_of,
            commands::graph_overlay,
            commands::investigate::investigate_log,
            commands::investigate::investigate_blame,
            commands::investigate::origin_candidates,
            commands::investigate::open_investigate_window,
            commands::commit_details,
            commands::commit_files,
            commands::diff_file,
            commands::worktree_files,
            commands::repo_status,
            commands::stage_paths,
            commands::unstage_paths,
            commands::discard_paths,
            commands::commit,
            commands::branches::checkout,
            commands::branches::create_branch,
            commands::branches::delete_branch,
            commands::command_log,
            commands::command_outcome,
            commands::close_this_window,
            commands::command_problems,
            commands::clear_command_log,
            commands::safety_log,
            commands::undo_last,
            commands::abort_operation,
            commands::continue_operation,
            commands::stash::stashes,
            commands::stash::stash_push,
            commands::toolbar::stash_keeping_worktree,
            commands::stash::stash_apply,
            commands::stash::stash_drop,
            commands::branches::create_tag,
            commands::branches::delete_tag,
            commands::ref_ops::reset_to,
            commands::ref_ops::is_ancestor,
            commands::ref_ops::compare_files,
            commands::ref_ops::tag_name_problem,
            commands::ref_ops::tag_message,
            commands::ref_ops::rename_tag,
            commands::ref_ops::rename_stash,
            commands::ref_ops::edit_author,
            commands::ref_ops::push_to,
            commands::network::remotes,
            commands::network::fetch,
            commands::network::pull,
            commands::network::push,
            commands::merge,
            commands::rebase,
            commands::skip_operation,
            commands::cherry_pick,
            commands::revert,
            commands::lost_commits,
            commands::repositories,
            commands::close_repository,
            commands::update_submodule,
            commands::remote_ops::submodule_op,
            commands::remote_ops::add_submodule,
            commands::remote_ops::subtree_op,
            commands::remote_ops::subtree_prefixes,
            commands::remote_ops::lfs_version,
            commands::remote_ops::lfs_op,
            commands::remote_ops::repo_settings,
            commands::remote_ops::write_repo_settings,
            commands::stage_selection,
            commands::blame,
            commands::open_blame_window,
            commands::line_history,
            commands::file_revisions,
            commands::network::remote_url,
            commands::ref_dates,
            commands::add_to_gitignore,
            commands::delete_untracked,
            commands::image_sides,
            commands::conflicts::conflicted_paths,
            commands::conflicts::conflict_text,
            commands::conflicts::resolve_conflict,
            commands::conflicts::resolve_conflict_text,
            commands::find_object,
            commands::branches::rename_branch,
            commands::branches::set_upstream,
            commands::branches::delete_remote_branch,
            commands::undo_entry,
            commands::stash::stash_contents,
            commands::stash::stash_selection,
            commands::hooks::run_check,
            commands::worktrees::worktrees,
            commands::worktrees::worktree_holding,
            commands::worktrees::add_worktree,
            commands::worktrees::remove_worktree,
            commands::worktrees::prune_worktrees,
            commands::worktrees::open_worktree,
            commands::worktrees::worktree_changes,
            commands::worktrees::prune_worktree,
            commands::worktrees::repair_worktree,
            commands::worktrees::lock_worktree,
            commands::worktrees::unlock_worktree,
            commands::flow::flow_status,
            commands::flow::flow_init,
            commands::flow::flow_start,
            commands::flow::flow_finish,
            commands::conflicts::merge_preview,
            commands::conflicts::open_merge_window,
            commands::conflicts::merge_resolved,
            commands::protecting_refs,
            commands::presets::export_preset,
            commands::presets::remove_preset,
            commands::avatars::avatars,
            commands::avatars::avatar_window,
            commands::avatars::set_avatars,
            commands::terminal_choices,
            commands::open_in_terminal,
            commands::desktop::desktop_info,
            commands::desktop::open_path,
            commands::desktop::reveal_path,
            commands::desktop::open_power_shell,
            commands::desktop::open_git_shell,
            commands::desktop::move_to_trash,
            commands::file_ops::remove_from_repository,
            commands::file_ops::move_path,
            commands::file_ops::set_index_flag,
            commands::file_ops::index_editor_sides,
            commands::file_ops::write_index_editor,
            commands::file_ops::save_blob,
            commands::file_ops::open_read_only,
            commands::file_ops::apply_commit_file,
            commands::file_ops::present_on_disk,
            commands::set_menu_state,
            commands::report_timing,
            commands::report_memory,
            commands::log_from_frontend,
            commands::closing_ping,
            commands::commit_tree_files,
            commands::search_file_contents,
            commands::list_submodules,
            commands::open_submodule,
            commands::repository_health,
            commands::read_git_config,
            commands::write_git_config,
            commands::cancel_operation,
            commands::list_operations,
            commands::read_settings,
            commands::write_setting,
            commands::default_keymap,
            commands::set_keymap,
            commands::scan_for_repositories,
            commands::network::has_token,
            commands::network::store_token,
            commands::network::forget_token,
            commands::hooks::list_hooks,
            commands::hooks::read_hook,
            commands::hooks::write_hook,
            commands::hooks::set_hook_enabled,
            commands::hooks::use_hooks_path,
            commands::hooks::run_hook,
            commands::rollback_to,
            commands::is_published,
            commands::toolbar::is_merged_into_head,
            commands::toolbar::delete_merged_branches,
            commands::split_off,
            commands::rebase_todo,
            commands::interactive_rebase,
            commands::rebase_progress,
            commands::overlap_window,
            commands::hooks::bypass_log,
            commands::popup_context_menu,
            commands::open_compare_window,
            commands::hooks::commit_template,
            commands::stage_mode,
            commands::presets::list_presets,
            commands::presets::install_preset,
            commands::diff_files,
            commands::file_before,
            commands::investigate,
            commands::discard_selection
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
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_filter(child_window::is_main)
                .build(),
        )
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .on_window_event(|window, event| {
            // A child window closing is not the app closing (R-201).
            if matches!(event, tauri::WindowEvent::CloseRequested { .. })
                && child_window::is_main(window.label())
            {
                shutdown::watch(window.app_handle());
            }
        })
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app| {
            let log_dir = app.path().app_log_dir()?;
            let config_dir = app.path().app_config_dir()?;
            let (guard, log_path) = logging::init(&log_dir, &config_dir)?;
            logging::install_panic_hook(&log_dir);
            webview_memory::spawn(std::process::id());

            tracing::info!(
                version = env!("CARGO_PKG_VERSION"),
                webview2 = webview2_version().as_deref().unwrap_or("unknown"),
                build = if cfg!(debug_assertions) { "debug" } else { "release" },
                log_dir = %log_dir.display(),
                "cogit starting"
            );

            let state = Arc::new(AppState::new());
            if let Ok(dir) = app.path().app_config_dir() {
                state.use_preset_dir(dir.join("presets"));
            }
            app.manage(AppContext {
                state: Arc::clone(&state),
                log_path,
                config_dir: config_dir.clone(),
            });
            app.manage(guard);
            app.manage(Arc::new(operations::Cancellations::default()));

            specta_builder.mount_events(app);
            events::forward_repo_changes(app.handle().clone(), &state);

            app.manage(menu::ContextMenu::<tauri::Wry>::default());
            let stored = menu::stored_keymap(&config_dir);
            let (menu, collected) = menu::build(app.handle(), &stored)?;
            app.set_menu(menu)?;
            app.manage(menu::MenuItems::from(collected));
            let keymap = menu::Keymap::default();
            keymap.set(stored);
            app.manage(keymap);
            app.on_menu_event(|app, event| dispatch_menu_command(app, &event.id().0));

            if let Some(window) = app.get_webview_window("main") {
                // Subscribed before the window is shown: a renderer that dies during the first
                // paint must not be the one failure nobody catches.
                #[cfg(windows)]
                renderer_failure::install(&window);
                #[cfg(windows)]
                session_end::install(&window);
                #[cfg(windows)]
                webview2::install_accelerators(&window);

                // Once, after the state plugin restored the saved geometry and before the
                // window is shown. Never again: Windows moves and resizes the window
                // itself for maximize, snap and minimize, and correcting it afterwards is
                // a fight the window loses (doc/12-risks.md, R-118).
                window_place::settle(&window);
                window.show()?;
            }
            Ok(())
        })
        .build(tauri::generate_context!())?
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
                shutdown::exiting(app);
            }
            if matches!(event, tauri::RunEvent::Exit) {
                git_engine::children::stop_all();
                tracing::info!(version = env!("CARGO_PKG_VERSION"), "cogit stopped");
            }
        });

    Ok(())
}

/// What a menu id does, wherever it came from: the bar, the command palette, or a key
/// the window took back from the webview (problem 3).
pub fn dispatch_menu_command(app: &tauri::AppHandle, id: &str) {
    if child_window::on_menu(app, id) {
        return;
    }
    if id == "copy-diagnostics" {
        copy_diagnostics(app);
        return;
    }
    if id == "reset-window-position" {
        if let Some(window) = app.get_webview_window("main") {
            window_place::recentre(&window);
        }
        return;
    }
    let _ = MenuCommand(id.to_owned()).emit(app);
}

/// `None` off Windows, where there is no WebView2 to ask about.
fn webview2_version() -> Option<String> {
    #[cfg(windows)]
    {
        webview2::browser_version()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// `Help ▸ Copy Diagnostics`. Answered in Rust so that it still works when the webview is
/// the part that stopped responding.
fn copy_diagnostics(app: &tauri::AppHandle) {
    use tauri_plugin_clipboard_manager::ClipboardExt as _;

    let Some(context) = app.try_state::<AppContext>() else {
        return;
    };
    let text = diagnostics::report(
        &context.log_path,
        &context.config_dir,
        webview2_version().as_deref(),
    );

    match app.clipboard().write_text(text) {
        Ok(()) => tracing::info!("diagnostics copied to the clipboard"),
        Err(err) => tracing::error!(error = %err, "cannot copy diagnostics"),
    }
}

#[cfg(debug_assertions)]
fn eprintln_fallback(message: &str) {
    use std::io::Write as _;
    let mut stderr = std::io::stderr();
    let _ = writeln!(stderr, "[cogit] {message}");
}
