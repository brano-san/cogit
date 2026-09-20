use std::collections::HashMap;
use std::sync::Mutex;

use tauri::menu::{
    CheckMenuItem, CheckMenuItemBuilder, Menu, MenuItem, MenuItemBuilder, PredefinedMenuItem,
    Submenu, SubmenuBuilder,
};
use tauri::{AppHandle, Manager, Runtime};

/// Item ids are the palette command ids: one place decides what an action is called and
/// when it is available, and both the menu and the palette read it.
const REPOSITORY: &[Entry] = &[
    Entry::Item("open", "Open Repository…", Some("CmdOrCtrl+O")),
    Entry::Item("close", "Close Repository", Some("CmdOrCtrl+W")),
    Entry::Separator,
    Entry::Item("refresh", "Refresh", Some("F5")),
    Entry::Separator,
    Entry::Item("settings", "Settings…", Some("CmdOrCtrl+,")),
];

const VIEW: &[Entry] = &[
    Entry::Check("output", "Output", Some("CmdOrCtrl+Shift+7")),
    Entry::Check("maximize-panel", "Maximise Panel", Some("Shift+F11")),
    Entry::Separator,
    Entry::Check(
        "panel-repositories",
        "Repositories Panel",
        Some("CmdOrCtrl+1"),
    ),
    Entry::Check("panel-refs", "References Panel", Some("CmdOrCtrl+2")),
    Entry::Check("panel-graph", "Graph Panel", Some("CmdOrCtrl+3")),
    Entry::Check("panel-files", "Files Panel", Some("CmdOrCtrl+4")),
    Entry::Check("panel-diff", "Diff Panel", Some("CmdOrCtrl+5")),
    Entry::Separator,
    Entry::Check("overlap", "Commit Overlap Column", None),
    Entry::Separator,
    Entry::Check("perspective-main", "Perspective: Main", None),
    Entry::Check("perspective-review", "Perspective: Review", None),
    Entry::Item("reset-layout", "Reset Perspective", None),
];

const REMOTE: &[Entry] = &[
    Entry::Item("fetch", "Fetch", Some("CmdOrCtrl+Shift+F")),
    Entry::Item("pull", "Pull", Some("CmdOrCtrl+Shift+U")),
    Entry::Item("push", "Push", Some("CmdOrCtrl+Shift+O")),
    Entry::Separator,
    Entry::Item("pr", "Create Pull Request", None),
];

const LOCAL: &[Entry] = &[
    Entry::Item("commit", "Commit…", Some("CmdOrCtrl+Return")),
    Entry::Item("stash", "Stash All", None),
    Entry::Separator,
    Entry::Item("undo", "Undo Last Operation", None),
    Entry::Item("abort", "Abort Operation In Progress", None),
    Entry::Separator,
    Entry::Item("rebase-i", "Rebase Commits After This One…", None),
    Entry::Item("split-off", "Split Off Files…", None),
    Entry::Item("rollback", "Roll Back Tree To This Commit", None),
];

const BRANCH: &[Entry] = &[
    Entry::Item("branch", "New Branch…", None),
    Entry::Item("tag", "Create Tag", None),
];

const QUERY: &[Entry] = &[
    Entry::Item("find", "Find Object…", Some("CmdOrCtrl+P")),
    Entry::Item("palette", "Find Command…", Some("CmdOrCtrl+Shift+P")),
    Entry::Separator,
    Entry::Item("blame", "Blame This File", None),
];

const TOOLS: &[Entry] = &[
    Entry::Item("hooks", "Manage Hooks…", None),
    Entry::Separator,
    Entry::Item("reveal-log", "Reveal Log File", None),
    Entry::Item("copy-pr", "Copy Pull Request Link", None),
];

const HELP: &[Entry] = &[Entry::Item("about", "About Cogit", None)];

enum Entry {
    Item(&'static str, &'static str, Option<&'static str>),
    /// A toggle. muda flips the tick itself on click, so the frontend always writes the
    /// authoritative state back through `set_menu_state` afterwards.
    Check(&'static str, &'static str, Option<&'static str>),
    Separator,
}

/// The items by id, so a state change can enable or disable one without walking the tree:
/// `Menu::get` only looks at the top level.
pub struct MenuItems<R: Runtime> {
    items: Mutex<HashMap<String, MenuItem<R>>>,
    checks: Mutex<HashMap<String, CheckMenuItem<R>>>,
}

impl<R: Runtime> MenuItems<R> {
    pub fn apply(&self, disabled: &[String], checked: &[String]) {
        if let Ok(items) = self.items.lock() {
            for (id, item) in items.iter() {
                report(id, item.set_enabled(!disabled.contains(id)));
            }
        }
        if let Ok(checks) = self.checks.lock() {
            for (id, item) in checks.iter() {
                report(id, item.set_enabled(!disabled.contains(id)));
                report(id, item.set_checked(checked.contains(id)));
            }
        }
    }
}

fn report(id: &str, result: tauri::Result<()>) {
    if let Err(err) = result {
        tracing::error!(error = ?err, id, context = "failed to update a menu item");
    }
}

struct Collected<R: Runtime> {
    items: HashMap<String, MenuItem<R>>,
    checks: HashMap<String, CheckMenuItem<R>>,
}

fn submenu<R: Runtime>(
    app: &AppHandle<R>,
    title: &str,
    entries: &[Entry],
    collected: &mut Collected<R>,
) -> tauri::Result<Submenu<R>> {
    let mut builder = SubmenuBuilder::new(app, title);
    for entry in entries {
        match entry {
            Entry::Separator => builder = builder.separator(),
            Entry::Item(id, label, accelerator) => {
                let mut item = MenuItemBuilder::with_id(*id, *label);
                if let Some(keys) = accelerator {
                    item = item.accelerator(*keys);
                }
                let item = item.build(app)?;
                builder = builder.item(&item);
                collected.items.insert((*id).to_owned(), item);
            }
            Entry::Check(id, label, accelerator) => {
                let mut item = CheckMenuItemBuilder::with_id(*id, *label);
                if let Some(keys) = accelerator {
                    item = item.accelerator(*keys);
                }
                let item = item.build(app)?;
                builder = builder.item(&item);
                collected.checks.insert((*id).to_owned(), item);
            }
        }
    }
    builder.build()
}

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let mut collected = Collected {
        items: HashMap::new(),
        checks: HashMap::new(),
    };
    let mut section = |title: &str, entries: &[Entry]| submenu(app, title, entries, &mut collected);

    let repository = section("Repository", REPOSITORY)?;
    let view = section("View", VIEW)?;
    let remote = section("Remote", REMOTE)?;
    let local = section("Local", LOCAL)?;
    let branch = section("Branch", BRANCH)?;
    let query = section("Query", QUERY)?;
    let tools = section("Tools", TOOLS)?;
    let help = section("Help", HELP)?;

    // Edit and Window are the OS's own items: the webview needs real clipboard entries
    // for Ctrl+C to work inside an input, and muda gives them native behaviour.
    let edit = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;
    let window = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .separator()
        .item(&PredefinedMenuItem::close_window(app, Some("Close"))?)
        .build()?;

    let menu = Menu::with_items(
        app,
        &[
            &repository,
            &edit,
            &view,
            &remote,
            &local,
            &branch,
            &query,
            &tools,
            &window,
            &help,
        ],
    )?;

    app.manage(MenuItems {
        items: Mutex::new(collected.items),
        checks: Mutex::new(collected.checks),
    });
    Ok(menu)
}

/// One row the frontend asks for in a context menu. Ids are palette command ids, so the
/// chosen item travels back through the same `menu-command` event as the menu bar.
#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContextItem {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    #[serde(default)]
    pub separator: bool,
}

/// Held until the next popup replaces it: dropping the menu closes it under the pointer.
pub struct ContextMenu<R: Runtime> {
    current: Mutex<Option<Menu<R>>>,
}

impl<R: Runtime> Default for ContextMenu<R> {
    fn default() -> Self {
        Self {
            current: Mutex::new(None),
        }
    }
}

pub fn popup<R: Runtime>(
    window: &tauri::Window<R>,
    held: &ContextMenu<R>,
    items: &[ContextItem],
    x: f64,
    y: f64,
) -> tauri::Result<()> {
    let app = window.app_handle();
    let mut builder = tauri::menu::MenuBuilder::new(app);
    for item in items {
        if item.separator {
            builder = builder.separator();
            continue;
        }
        let entry = MenuItemBuilder::with_id(item.id.as_str(), item.label.as_str())
            .enabled(item.enabled)
            .build(app)?;
        builder = builder.item(&entry);
    }

    let menu = builder.build()?;
    window.popup_menu_at(&menu, tauri::LogicalPosition::new(x, y))?;
    if let Ok(mut slot) = held.current.lock() {
        *slot = Some(menu);
    }
    Ok(())
}
