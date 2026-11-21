use super::Entry;
use super::macos::{APP_ID, EDIT_ID, section_entries};
use crate::child_window;
use std::collections::HashMap;
use tauri::menu::{Menu, MenuItem, Submenu, SubmenuBuilder};
use tauri::{AppHandle, Manager, Runtime};

pub(crate) struct WindowMenus<R: Runtime>(parking_lot::Mutex<HashMap<String, WindowMenu<R>>>);

struct WindowMenu<R: Runtime> {
    menu: Option<Menu<R>>,
    context: Option<EditContext>,
}

impl<R: Runtime> Default for WindowMenu<R> {
    fn default() -> Self {
        Self {
            menu: None,
            context: None,
        }
    }
}

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct EditContext {
    label: String,
    native_select_all: bool,
    native_undo: bool,
}

pub(crate) fn app_command(app: &AppHandle, id: &str) -> bool {
    use tauri_specta::Event as _;
    if !matches!(
        id,
        "about" | "check-updates" | "settings" | "exit" | "reveal-log"
    ) {
        return false;
    }
    if let Some(main) = app.get_webview_window(child_window::MAIN) {
        if let Err(error) = main
            .unminimize()
            .and_then(|()| main.set_focus())
            .and_then(|()| crate::MenuCommand(id.to_owned()).emit_to(app, child_window::MAIN))
        {
            tracing::warn!(%error, id, "cannot run the application menu command");
        }
    } else if id == "exit" {
        for window in app.webview_windows().into_values() {
            if let Err(error) = window.close() {
                tracing::warn!(%error, window = window.label(), "cannot close the remaining window");
            }
        }
    }
    true
}

impl<R: Runtime> WindowMenus<R> {
    pub(crate) fn new(main: Menu<R>) -> Self {
        Self(parking_lot::Mutex::new(HashMap::from([(
            child_window::MAIN.to_owned(),
            WindowMenu {
                menu: Some(main),
                context: None,
            },
        )])))
    }

    pub(crate) fn remember(&self, label: &str, menu: Menu<R>) {
        let mut menus = self.0.lock();
        menus.entry(label.to_owned()).or_default().menu = Some(menu);
    }

    pub(crate) fn forget(&self, label: &str) {
        self.0.lock().remove(label);
    }

    pub(crate) fn rebuild_main(&self, app: &AppHandle<R>, menu: Menu<R>) -> tauri::Result<()> {
        let current = app.menu();
        let label = self.0.lock().iter().find_map(|(label, slot)| {
            current
                .as_ref()
                .zip(slot.menu.as_ref())
                .filter(|(a, b)| a.id() == b.id())
                .map(|_| label.clone())
        });
        self.remember(child_window::MAIN, menu);
        self.activate(app, label.as_deref().unwrap_or(child_window::MAIN), true)
    }

    fn activate(&self, app: &AppHandle<R>, label: &str, rebuilt: bool) -> tauri::Result<()> {
        let (menu, context, main) = {
            let windows = self.0.lock();
            let Some(slot) = windows.get(label) else {
                return Ok(());
            };
            (
                slot.menu.clone(),
                slot.context.clone(),
                windows
                    .get(child_window::MAIN)
                    .and_then(|slot| slot.menu.clone()),
            )
        };
        let Some(menu) = menu else { return Ok(()) };
        let active = app.menu();
        if !rebuilt && active.as_ref().is_some_and(|bar| bar.id() == menu.id()) {
            return Ok(());
        }
        if let Some(context) = context {
            update_edit(app, &menu, &context)?;
        }
        if let Some(application) = main
            .as_ref()
            .or(active.as_ref())
            .and_then(|bar| bar.get(APP_ID))
        {
            if let Some(old) = menu.get(APP_ID) {
                menu.remove(&old)?;
            }
            menu.prepend(&application)?;
        }
        app.set_menu(menu)?;
        Ok(())
    }
}

pub(crate) fn listen<R: Runtime>(app: &AppHandle<R>) {
    use tauri::Listener;
    let handle = app.clone();
    app.listen("mac-edit-context", move |event| {
        let Ok(context) = serde_json::from_str::<EditContext>(event.payload()) else {
            return;
        };
        let app = handle.clone();
        if let Err(error) = handle.run_on_main_thread(move || {
            let menus = app.state::<WindowMenus<R>>();
            if app.get_webview_window(&context.label).is_none() {
                return;
            }
            let menu = {
                let mut windows = menus.0.lock();
                let slot = windows.entry(context.label.clone()).or_default();
                slot.context = Some(context.clone());
                slot.menu.clone()
            };
            if let Some(menu) = menu
                && let Err(error) = update_edit(&app, &menu, &context)
            {
                tracing::warn!(%error, window = context.label, "cannot update the focused Edit commands");
            }
        }) {
            tracing::warn!(%error, "cannot schedule an Edit menu update");
        }
    });
}

fn update_edit<R: Runtime>(
    app: &AppHandle<R>,
    menu: &Menu<R>,
    context: &EditContext,
) -> tauri::Result<()> {
    use tauri::menu::{MenuItemKind, PredefinedMenuItem};
    let Some(MenuItemKind::Submenu(edit)) = menu.get(EDIT_ID) else {
        return Ok(());
    };
    let items = edit.items()?;
    for (index, native, label, action, keys) in [
        (0, context.native_undo, "Undo", "edit-undo", "CmdOrCtrl+Z"),
        (
            1,
            context.native_undo,
            "Redo",
            "edit-redo",
            "CmdOrCtrl+Shift+Z",
        ),
        (
            6,
            context.native_select_all,
            "Select All",
            "select-all",
            "CmdOrCtrl+A",
        ),
    ] {
        let Some(current) = items.get(index) else {
            continue;
        };
        if matches!(current, MenuItemKind::Predefined(_)) == native {
            continue;
        }
        if native {
            let item = match action {
                "edit-undo" => PredefinedMenuItem::undo(app, None)?,
                "edit-redo" => PredefinedMenuItem::redo(app, None)?,
                _ => PredefinedMenuItem::select_all(app, None)?,
            };
            edit.insert(&item, index)?;
        } else {
            let main = child_window::is_main(&context.label);
            let id = if action == "select-all" && main {
                action.to_owned()
            } else {
                child_window::menu_id(&context.label, action)
            };
            let label = if main && action == "edit-undo" {
                "Discard Changes…"
            } else {
                label
            };
            let enabled = !main || action != "edit-redo";
            edit.insert(
                &MenuItem::with_id(app, id, label, enabled, Some(keys))?,
                index,
            )?;
        }
        edit.remove(current)?;
    }
    Ok(())
}

pub(crate) fn event<R: Runtime>(window: &tauri::Window<R>, event: &tauri::WindowEvent) {
    let app = window.app_handle();
    let Some(menus) = app.try_state::<WindowMenus<R>>() else {
        return;
    };
    match event {
        tauri::WindowEvent::Focused(true) => {
            if let Err(error) = menus.activate(app, window.label(), false) {
                tracing::warn!(%error, window = window.label(), "cannot activate the window menu");
            }
        }
        tauri::WindowEvent::Destroyed => menus.forget(window.label()),
        _ => {}
    }
}

pub(crate) fn child_menu<R: Runtime>(
    app: &AppHandle<R>,
    label: &str,
    sections: &[child_window::Submenu],
) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app)?;
    let edit = SubmenuBuilder::with_id(app, EDIT_ID, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;
    menu.append(&edit)?;
    let window = SubmenuBuilder::with_id(app, tauri::menu::WINDOW_SUBMENU_ID, "Window")
        .minimize()
        .maximize_with_text("Zoom")
        .close_window()
        .separator()
        .bring_all_to_front()
        .build()?;
    let help = Submenu::with_id(app, tauri::menu::HELP_SUBMENU_ID, "Help", true)?;
    for section in sections {
        let mut items = section
            .items
            .iter()
            .filter(|item| item.action != child_window::CLOSE)
            .peekable();
        if items.peek().is_none() {
            continue;
        }
        let parent = match section.title {
            "Edit" => {
                edit.append(&tauri::menu::PredefinedMenuItem::separator(app)?)?;
                edit.clone()
            }
            "Window" => {
                window.append(&tauri::menu::PredefinedMenuItem::separator(app)?)?;
                window.clone()
            }
            "Help" => help.clone(),
            title => {
                let submenu = Submenu::new(app, title, true)?;
                menu.append(&submenu)?;
                submenu
            }
        };
        for item in items {
            parent.append(&MenuItem::with_id(
                app,
                child_window::menu_id(label, item.action),
                item.label,
                true,
                item.accelerator,
            )?)?;
        }
    }
    let help_entries = section_entries(true, super::HELP);
    if !help.items()?.is_empty() {
        help.append(&tauri::menu::PredefinedMenuItem::separator(app)?)?;
    }
    for entry in super::tidy(&help_entries) {
        if let Entry::Item(id, text, _) = entry {
            help.append(&MenuItem::with_id(app, *id, *text, true, None::<&str>)?)?;
        }
    }
    menu.append(&window)?;
    menu.append(&help)?;
    Ok(menu)
}
