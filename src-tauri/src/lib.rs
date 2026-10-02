mod accelerators;
mod blame_window;
mod child_window;
mod commands;
mod commit_window;
mod diagnostics;
mod errors_window;
mod events;
mod investigate_window;
mod key_capture;
mod logging;
mod menu;
mod native_theme;
mod portable_mode;
#[cfg_attr(not(feature = "portable"), allow(dead_code))]
mod portable_window_state;
mod profile;
mod recycle_bin;
#[cfg(windows)]
mod renderer_failure;
#[cfg(windows)]
mod session_end;
mod shutdown;
mod solver_window;
mod taskbar;
#[cfg(windows)]
mod webview2;
mod webview2_check;
mod webview_memory;
mod window_chrome;
mod window_place;

use app_state::AppState;
pub use events::{
    AvatarReady, CommandRecorded, ErrorQueue, ErrorReported, ErrorsAction, MenuCommand,
    MergeResolved, MergeToolFinished, OpenModule, OperationChanged, RepoChanged, RevealCommit,
    SessionEnding, SettingsChanged, TreeChanged,
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

/// Before the first git runs: Preferences ▸ Git executable is read once, at startup.
fn use_git_from_settings(config_dir: &std::path::Path) {
    let Some(program) = app_state::settings::read_git_program(config_dir) else {
        return;
    };
    git_engine::use_git_program(program.clone());
    match git_engine::git_version() {
        Ok(version) => {
            tracing::info!(program = %program.display(), %version, "git from Preferences")
        }
        Err(err) => {
            tracing::error!(error = ?err, context = "the git set in Preferences does not run")
        }
    }
}

/// Temp copies of an earlier run's merge tool sides: a crash left them behind.
fn sweep_merge_temp() {
    const STALE: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);
    let removed =
        app_state::merge_tool::sweep(&std::env::temp_dir(), STALE, std::time::SystemTime::now());
    if removed > 0 {
        tracing::info!(removed, "stale merge tool temp folders removed");
    }
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
            MergeToolFinished,
            CommandRecorded,
            SessionEnding,
            RevealCommit,
            ErrorReported,
            ErrorQueue,
            ErrorsAction,
            SettingsChanged,
            OpenModule,
            TreeChanged
        ])
        .commands(collect_commands![
            commands::app_info,
            commands::probe_git,
            commands::find_git_candidates,
            commands::use_git,
            commands::open_third_party_licences,
            commands::open_repository,
            commands::reread_repository,
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
            commands::working_state,
            commands::repo_refs,
            commands::stage_paths,
            commands::stage_all,
            commands::unstage_paths,
            commands::discard_paths,
            commands::commit,
            commands::branches::checkout,
            commands::branches::switch_with_autostash,
            commands::branches::create_branch,
            commands::branches::delete_branch,
            commands::command_log,
            commands::command_log_text,
            commands::command_outcome,
            commands::close_this_window,
            commands::open_errors_window,
            commands::focus_main_window,
            commands::command_problems,
            commands::clear_command_log,
            commands::safety_log,
            commands::abort_operation,
            commands::continue_operation,
            commands::remove_index_lock,
            commands::bisect::bisect_start,
            commands::bisect::bisect_mark,
            commands::bisect::bisect_reset,
            commands::stash::stashes,
            commands::stash::stash_push,
            commands::toolbar::stash_keeping_worktree,
            commands::stash::stash_apply,
            commands::stash::stash_drop,
            commands::branches::create_tag,
            commands::branches::delete_tag,
            commands::branches::delete_remote_tag,
            commands::ref_ops::reset_to,
            commands::ref_ops::undo_rewrite,
            commands::ref_ops::undo_rewrite_info,
            commands::ref_ops::is_ancestor,
            commands::ref_ops::compare_files,
            commands::ref_ops::tag_name_problem,
            commands::ref_ops::tag_message,
            commands::ref_ops::rename_tag,
            commands::ref_ops::rename_stash,
            commands::ref_ops::noted_commits,
            commands::ref_ops::commit_notes,
            commands::ref_ops::set_note,
            commands::ref_ops::edit_author,
            commands::ref_ops::push_to,
            commands::network::remotes,
            commands::network::fetch,
            commands::network::pull,
            commands::network::push,
            commands::network::fetch_with,
            commands::network::pull_with,
            commands::network::push_with,
            commands::network::push_notes,
            commands::network::merge_notes,
            commands::network::push_preview,
            commands::network::network_defaults,
            commands::network::save_network_defaults,
            commands::network::cancel_network,
            commands::remotes::remote_info,
            commands::remotes::rename_remote,
            commands::remotes::remove_remote,
            commands::remotes::set_remote_properties,
            commands::remotes::fetch_more,
            commands::remotes::fetch_depth,
            commands::merge,
            commands::rebase,
            commands::skip_operation,
            commands::cherry_pick,
            commands::revert,
            commands::lost_commits,
            commands::repositories,
            commands::close_repository,
            commands::show_repository,
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
            commands::other_refs,
            commands::add_to_gitignore,
            commands::image_sides,
            commands::conflicts::conflicted_paths,
            commands::conflicts::conflict_text,
            commands::conflicts::resolve_conflict,
            commands::conflicts::resolve_conflict_text,
            commands::find_object,
            commands::branches::rename_branch,
            commands::branches::branch_reflog,
            commands::branches::restore_branch,
            commands::branches::set_upstream,
            commands::branches::delete_remote_branch,
            commands::undo_entry,
            commands::stash::stash_contents,
            commands::stash::stash_selection,
            commands::hooks::run_check,
            commands::checkup::run_maintenance,
            commands::checkup::add_to_exclude,
            commands::checkup::ignore_rules,
            commands::checkup::lfs_locks,
            commands::checkup::lfs_file_states,
            commands::checkup::commit_signature,
            commands::checkup::unportable_paths,
            commands::checkup::rerere_status,
            commands::checkup::rerere_forget,
            commands::checkup::range_diff,
            commands::worktrees::worktrees,
            commands::worktrees::worktree_holding,
            commands::worktrees::add_worktree,
            commands::worktrees::check_revision,
            commands::worktrees::check_branch_name,
            commands::worktrees::worktree_folder_problem,
            commands::worktrees::remove_worktree,
            commands::worktrees::prune_worktrees,
            commands::branches::delete_refs,
            commands::worktrees::worktree_leftover,
            commands::worktrees::delete_worktree_leftover,
            commands::worktrees::open_worktree,
            commands::worktrees::scan_worktree_removal,
            commands::worktrees::prune_worktree,
            commands::worktrees::repair_worktree,
            commands::worktrees::lock_worktree,
            commands::worktrees::unlock_worktree,
            commands::flow::flow_status,
            commands::flow::flow_init,
            commands::flow::flow_start,
            commands::flow::flow_finish,
            commands::conflicts::merge_preview,
            commands::conflicts::open_solver_window,
            commands::conflicts::solver_data,
            commands::conflicts::mark_conflict_resolved,
            commands::conflicts::launch_merge_tool,
            commands::conflicts::cancel_merge_tool,
            commands::conflicts::merge_tools_running,
            commands::protecting_refs,
            commands::presets::export_preset,
            commands::presets::remove_preset,
            commands::avatars::avatars,
            commands::avatars::avatar_window,
            commands::avatars::set_avatars,
            commands::terminal_choices,
            commands::open_in_terminal,
            commands::desktop::desktop_info,
            commands::desktop::set_native_theme,
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
            commands::set_taskbar_state,
            commands::report_timing,
            commands::report_memory,
            commands::log_from_frontend,
            commands::closing_ping,
            commands::commit_tree_files,
            commands::search_file_contents,
            commands::list_submodules,
            commands::repo_rows::submodule_outline,
            commands::repo_rows::repo_pulse,
            commands::repo_rows::pull_probe,
            commands::open_submodule,
            commands::repository_health,
            commands::trust_directory,
            commands::read_git_config,
            commands::write_git_config,
            commands::cancel_operation,
            commands::list_operations,
            commands::read_settings,
            commands::read_user_theme,
            commands::write_setting,
            commands::default_keymap,
            commands::set_keymap,
            commands::capture_keys,
            commands::scan_for_repositories,
            commands::clone::remote_branches,
            commands::clone::clone_destination,
            commands::clone::folder_kind,
            commands::clone::init_repository,
            commands::clone::clipboard_repository_url,
            commands::clone::clone_repository,
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
            commands::menu_model,
            commands::menu_command,
            commands::window_chrome,
            commands::open_compare_window,
            commands::commit_window::open_commit_window,
            commands::commit_window::recent_commits,
            commands::hooks::commit_template,
            commands::stage_mode,
            commands::presets::list_presets,
            commands::presets::install_preset,
            commands::diff_files,
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
    // First: the environment is read once by everything that follows (portable build only).
    let portable = portable_mode::activate()?;
    // Also before GTK: no session bus (WSLg) means dconf warns on every settings write.
    #[cfg(target_os = "linux")]
    portable::quiet_gsettings_without_a_bus();

    // Before the builder: without the runtime no window can exist, so the user is told here.
    #[cfg(windows)]
    webview2_check::require();

    // Before GTK starts: the Wayland app_id and the X11 WM_CLASS follow the program name,
    // and the shell matches them to `cogit.desktop` for the icon.
    #[cfg(target_os = "linux")]
    gtk::glib::set_prgname(Some(desktop_entry::APP_NAME));

    // Before the webview exists: its profile lives in the folder being renamed. A portable
    // build has no earlier install to carry over and touches no system folder.
    let migrated = if portable.is_some() {
        Vec::new()
    } else {
        app_state::legacy_dirs::migrate_legacy_dirs(&app_state::legacy_dirs::app_folder_pairs(
            app_state::legacy_dirs::LEGACY_IDENTIFIER,
            app_state::legacy_dirs::IDENTIFIER,
        ))
    };

    let specta_builder = specta_builder();

    #[cfg(debug_assertions)]
    if let Err(err) = specta_builder.export(Typescript::default(), BINDINGS_PATH) {
        eprintln_fallback(&format!("failed to export IPC bindings: {err}"));
    }

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init());
    // Both write to the system's folders (the plugin creates the config folder when it
    // saves; an update replaces the installed binary): a portable build has its own.
    if portable.is_none() {
        builder = builder
            .plugin(
                // Without VISIBLE: the plugin would show the window as it is created, before
                // `setup` settles its geometry and subscribes to its failures (R-113, R-118).
                tauri_plugin_window_state::Builder::new()
                    .with_filter(child_window::is_main)
                    .with_state_flags(
                        tauri_plugin_window_state::StateFlags::all()
                            - tauri_plugin_window_state::StateFlags::VISIBLE
                            // Decided by `window_chrome`, not remembered.
                            - tauri_plugin_window_state::StateFlags::DECORATIONS,
                    )
                    .build(),
            )
            .plugin(tauri_plugin_updater::Builder::new().build());
    }
    let mut context = tauri::generate_context!();
    if portable.is_some() {
        portable_mode::hold_config_windows(&mut context);
    }
    builder
        .plugin(tauri_plugin_process::init())
        .on_window_event(|window, event| {
            // A child window closing is not the app closing (R-201).
            if matches!(event, tauri::WindowEvent::CloseRequested { .. })
                && child_window::is_main(window.label())
            {
                shutdown::watch(window.app_handle());
            }
            // The main window gone, the app goes: a Blame window left open kept the process
            // alive without it. Each child is asked, so Merge still asks about its choices.
            if matches!(event, tauri::WindowEvent::Destroyed) && child_window::is_main(window.label()) {
                for (label, child) in window.app_handle().webview_windows() {
                    if !child_window::is_main(&label)
                        && let Err(err) = child.close()
                    {
                        tracing::warn!(error = %err, window = %label, "cannot close a child window after the main one");
                    }
                }
            }
        })
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app| {
            #[cfg(target_os = "linux")]
            gtk::Window::set_default_icon_name(desktop_entry::APP_NAME);
            let (log_dir, config_dir) = match portable {
                Some(layout) => (layout.logs(), layout.config()),
                None => (app.path().app_log_dir()?, app.path().app_config_dir()?),
            };
            let (guard, log_path) = logging::init(&log_dir, &config_dir)?;
            logging::install_panic_hook(&log_dir);
            app_state::legacy_dirs::log_outcomes(&migrated);
            webview_memory::spawn(std::process::id());
            use_git_from_settings(&config_dir);
            sweep_merge_temp();

            tracing::info!(
                version = env!("CARGO_PKG_VERSION"),
                portable = portable.map(|layout| layout.root().display().to_string()),
                webview2 = webview2_version().as_deref().unwrap_or("unknown"),
                build = if cfg!(debug_assertions) { "debug" } else { "release" },
                log_dir = %log_dir.display(),
                "cogit starting"
            );

            let state = Arc::new(AppState::new());
            state.use_preset_dir(config_dir.join("presets"));
            app.manage(AppContext {
                state: Arc::clone(&state),
                log_path,
                config_dir: config_dir.clone(),
            });
            app.manage(logging::LogGuard::new(guard));

            specta_builder.mount_events(app);
            events::forward_repo_changes(app.handle().clone(), &state);

            let chrome = window_chrome::stored(&config_dir);
            app.manage(chrome);
            app.manage(menu::ContextMenu::<tauri::Wry>::default());
            let stored = menu::stored_keymap(&config_dir);
            let (menu, collected) = menu::build(app.handle(), &stored)?;
            app.set_menu(menu)?;
            app.manage(menu::MenuItems::from(collected));
            let keymap = menu::Keymap::default();
            keymap.set(stored);
            app.manage(keymap);
            app.manage(key_capture::KeyCapture::default());
            app.on_menu_event(|app, event| dispatch_menu_command(app, &event.id().0));

            if let Some(layout) = portable {
                portable_mode::create_config_windows(app, layout)?;
                if let Some(window) = app.get_webview_window(child_window::MAIN) {
                    portable_window_state::install(
                        &window,
                        layout.config().join(portable_window_state::FILE),
                    );
                }
            }

            if let Some(window) = app.get_webview_window("main") {
                // Subscribed before the window is shown: a renderer that dies during the first
                // paint must not be the one failure nobody catches.
                #[cfg(windows)]
                renderer_failure::install(&window);
                #[cfg(windows)]
                session_end::install(&window);
                #[cfg(windows)]
                webview2::install_accelerators(&window);
                #[cfg(windows)]
                webview2::harden(&window);

                // Once, after the state plugin restored the saved geometry and before the
                // window is shown. Never again: Windows moves and resizes the window
                // itself for maximize, snap and minimize, and correcting it afterwards is
                // a fight the window loses (doc/12-risks.md, R-118).
                window_place::settle(&window);
                if chrome.custom_titlebar {
                    window.set_decorations(false)?;
                }
                window.show()?;
                // After `show`, which makes every widget of the window visible again.
                if chrome.web_menus {
                    window_chrome::hide_native_menu(&window);
                }
            }
            Ok(())
        })
        .build(context)?
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
                shutdown::exiting(app);
            }
            if matches!(event, tauri::RunEvent::Exit) {
                git_engine::children::stop_all();
                tracing::info!(version = env!("CARGO_PKG_VERSION"), "cogit stopped");
                if let Some(guard) = app.try_state::<logging::LogGuard>() {
                    guard.finish();
                }
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
