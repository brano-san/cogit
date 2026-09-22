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
    Entry::Item("scan", "Scan Folder for Repositories…", None),
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
    Entry::Check("panel-commit", "Commit Message Panel", Some("CmdOrCtrl+5")),
    Entry::Check("panel-diff", "Diff Panel", Some("CmdOrCtrl+6")),
    Entry::Separator,
    Entry::Check("overlap", "Commit Overlap Column", None),
    Entry::Check("avatars", "Author Avatars", None),
    Entry::Separator,
    Entry::Check("perspective-main", "Perspective: Main", None),
    Entry::Check("perspective-review", "Perspective: Review", None),
    Entry::Item("reset-layout", "Reset Perspective", None),
];

const REMOTE: &[Entry] = &[
    Entry::Item("fetch", "Fetch", Some("CmdOrCtrl+Shift+F")),
    Entry::Item("fetch-all", "Fetch All", None),
    Entry::Item("pull", "Pull", Some("CmdOrCtrl+Shift+U")),
    Entry::Item("push", "Push", Some("CmdOrCtrl+Shift+O")),
    Entry::Separator,
    Entry::Item("pr", "Create Pull Request", None),
];

const LOCAL: &[Entry] = &[
    Entry::Item("commit", "Commit…", Some("CmdOrCtrl+Return")),
    Entry::Item("stash", "Stash All", Some("CmdOrCtrl+S")),
    Entry::Item(
        "stash-selection",
        "Stash Selection",
        Some("CmdOrCtrl+Alt+S"),
    ),
    Entry::Separator,
    Entry::Item("undo", "Undo Last Operation", None),
    Entry::Item("journal", "Safety Journal…", None),
    Entry::Item("abort", "Abort Operation In Progress", None),
    Entry::Separator,
    Entry::Separator,
    Entry::Item("flow-init", "Git-Flow: Set Up", None),
    Entry::Item("flow-feature", "Git-Flow: Start Feature…", None),
    Entry::Item("flow-release", "Git-Flow: Start Release…", None),
    Entry::Item("flow-hotfix", "Git-Flow: Start Hotfix…", None),
    Entry::Item("flow-finish", "Git-Flow: Finish This Branch…", None),
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
    Entry::Item("copy-pr", "Copy Pull Request Link", None),
];

const HELP: &[Entry] = &[
    // Moved here from Tools: this is where someone looks when asked to send a log.
    Entry::Item("reveal-log", "Open Log Folder", None),
    Entry::Item("copy-diagnostics", "Copy Diagnostics", None),
    Entry::Separator,
    Entry::Item("check-updates", "Check for Updates…", None),
    Entry::Separator,
    Entry::Item("about", "About Cogit", None),
];

/// The menu bar in order. One list, so the keymap editor and the menu cannot disagree.
const SECTIONS: &[(&str, &[Entry])] = &[
    ("Repository", REPOSITORY),
    ("View", VIEW),
    ("Remote", REMOTE),
    ("Local", LOCAL),
    ("Branch", BRANCH),
    ("Query", QUERY),
    ("Tools", TOOLS),
    ("Help", HELP),
];

/// One row of the keymap editor.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KeyBinding {
    pub id: String,
    pub label: String,
    pub section: String,
    /// What the menu ships with; the user's override lives in settings, not here.
    pub default_accelerator: Option<String>,
}

#[must_use]
pub fn default_keymap() -> Vec<KeyBinding> {
    let mut rows = Vec::new();
    for (section, entries) in SECTIONS {
        for entry in *entries {
            let (id, label, accelerator) = match entry {
                Entry::Item(id, label, keys) | Entry::Check(id, label, keys) => (id, label, keys),
                Entry::Separator => continue,
            };
            rows.push(KeyBinding {
                id: (*id).to_owned(),
                label: (*label).to_owned(),
                section: (*section).to_owned(),
                default_accelerator: accelerator.map(str::to_owned),
            });
        }
    }
    rows
}

/// `(id, default accelerator)` for every entry, in menu order.
///
/// Feeds the window-level accelerator table: one list decides what the bar shows and what
/// the window claims, so the two cannot drift apart (problem 3).
#[must_use]
pub fn default_keymap_pairs() -> Vec<(&'static str, Option<&'static str>)> {
    let mut rows = Vec::new();
    for (_, entries) in SECTIONS {
        for entry in *entries {
            match entry {
                Entry::Item(id, _, keys) | Entry::Check(id, _, keys) => rows.push((*id, *keys)),
                Entry::Separator => {}
            }
        }
    }
    rows
}

/// The user's overrides, held so a menu rebuild keeps them.
#[derive(Default)]
pub struct Keymap {
    overrides: Mutex<HashMap<String, String>>,
}

impl Keymap {
    pub fn set(&self, overrides: HashMap<String, String>) {
        if let Ok(mut held) = self.overrides.lock() {
            *held = overrides;
        }
    }

    #[must_use]
    pub fn current(&self) -> HashMap<String, String> {
        self.snapshot()
    }

    fn snapshot(&self) -> HashMap<String, String> {
        self.overrides
            .lock()
            .map(|held| held.clone())
            .unwrap_or_default()
    }
}

/// Read straight from the settings file: the bar is built before the webview exists, so
/// a restart must already show the user's own keys.
#[must_use]
pub fn stored_keymap(config_dir: &std::path::Path) -> HashMap<String, String> {
    app_state::settings::read_document(config_dir)
        .get("keymap")
        .and_then(serde_json::Value::as_object)
        .map(|map| {
            map.iter()
                .filter_map(|(id, keys)| Some((id.clone(), keys.as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default()
}

/// An empty override means "no accelerator at all", which is how a user removes one.
fn accelerator<'a>(
    overrides: &'a HashMap<String, String>,
    id: &str,
    fallback: Option<&'a &'static str>,
) -> Option<&'a str> {
    match overrides.get(id) {
        Some(keys) if keys.is_empty() => None,
        Some(keys) => Some(keys.as_str()),
        None => fallback.copied(),
    }
}

pub(crate) enum Entry {
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
    /// A rebuilt bar has new items; the old handles point at nothing the user can see.
    fn replace(&self, collected: Collected<R>) {
        if let Ok(mut items) = self.items.lock() {
            *items = collected.items;
        }
        if let Ok(mut checks) = self.checks.lock() {
            *checks = collected.checks;
        }
    }

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

pub struct Collected<R: Runtime> {
    items: HashMap<String, MenuItem<R>>,
    checks: HashMap<String, CheckMenuItem<R>>,
}

fn submenu<R: Runtime>(
    app: &AppHandle<R>,
    title: &str,
    entries: &[Entry],
    overrides: &HashMap<String, String>,
    collected: &mut Collected<R>,
) -> tauri::Result<Submenu<R>> {
    let mut builder = SubmenuBuilder::new(app, title);
    for entry in tidy(entries) {
        match entry {
            Entry::Separator => builder = builder.separator(),
            Entry::Item(id, label, fallback) => {
                let mut item = MenuItemBuilder::with_id(*id, *label);
                if let Some(keys) = accelerator(overrides, id, fallback.as_ref()) {
                    item = item.accelerator(keys);
                }
                let item = item.build(app)?;
                builder = builder.item(&item);
                collected.items.insert((*id).to_owned(), item);
            }
            Entry::Check(id, label, fallback) => {
                let mut item = CheckMenuItemBuilder::with_id(*id, *label);
                if let Some(keys) = accelerator(overrides, id, fallback.as_ref()) {
                    item = item.accelerator(keys);
                }
                let item = item.build(app)?;
                builder = builder.item(&item);
                collected.checks.insert((*id).to_owned(), item);
            }
        }
    }
    builder.build()
}

pub fn build<R: Runtime>(
    app: &AppHandle<R>,
    overrides: &HashMap<String, String>,
) -> tauri::Result<(Menu<R>, Collected<R>)> {
    let mut collected = Collected {
        items: HashMap::new(),
        checks: HashMap::new(),
    };
    let mut section =
        |title: &str, entries: &[Entry]| submenu(app, title, entries, overrides, &mut collected);

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
        .item(&MenuItem::with_id(
            app,
            "reset-window-position",
            "Reset Window Position",
            true,
            None::<&str>,
        )?)
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

    Ok((menu, collected))
}

impl<R: Runtime> From<Collected<R>> for MenuItems<R> {
    fn from(collected: Collected<R>) -> Self {
        Self {
            items: Mutex::new(collected.items),
            checks: Mutex::new(collected.checks),
        }
    }
}

/// Rebuilds the whole bar: muda cannot change an accelerator after an item is built, so the
/// held item handles are replaced along with it.
pub fn rebuild<R: Runtime>(
    app: &AppHandle<R>,
    keymap: &Keymap,
    items: &MenuItems<R>,
    overrides: HashMap<String, String>,
) -> tauri::Result<()> {
    keymap.set(overrides);
    let (menu, collected) = build(app, &keymap.snapshot())?;
    app.set_menu(menu)?;
    items.replace(collected);
    Ok(())
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
    /// Shown on the right of the row. A popup menu only draws it — the chord itself is
    /// bound in the frontend, which is the only place that knows the focused panel.
    #[serde(default)]
    pub accelerator: Option<String>,
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
        let mut entry =
            MenuItemBuilder::with_id(item.id.as_str(), item.label.as_str()).enabled(item.enabled);
        if let Some(chord) = &item.accelerator {
            entry = entry.accelerator(chord.as_str());
        }
        let entry = entry.build(app)?;
        builder = builder.item(&entry);
    }

    let menu = builder.build()?;
    window.popup_menu_at(&menu, tauri::LogicalPosition::new(x, y))?;
    if let Ok(mut slot) = held.current.lock() {
        *slot = Some(menu);
    }
    Ok(())
}

/// Drops separators that separate nothing: repeated ones, and one at either end. A menu
/// grows an entry at a time and the rule has to live with the builder, not be applied by
/// hand each time a line is added (doc/12-risks.md, R-133).
#[must_use]
pub fn tidy(entries: &[Entry]) -> Vec<&Entry> {
    let mut kept: Vec<&Entry> = Vec::with_capacity(entries.len());
    for entry in entries {
        if matches!(entry, Entry::Separator)
            && kept
                .last()
                .is_none_or(|last| matches!(last, Entry::Separator))
        {
            continue;
        }
        kept.push(entry);
    }
    while kept
        .last()
        .is_some_and(|last| matches!(last, Entry::Separator))
    {
        kept.pop();
    }
    kept
}

#[cfg(test)]
mod separator_tests {
    use super::*;

    const ITEM: Entry = Entry::Item("a", "A", None);
    const OTHER: Entry = Entry::Item("b", "B", None);

    fn kinds(entries: &[Entry]) -> Vec<bool> {
        tidy(entries)
            .into_iter()
            .map(|entry| matches!(entry, Entry::Separator))
            .collect()
    }

    #[test]
    fn two_separators_in_a_row_become_one() {
        assert_eq!(
            kinds(&[ITEM, Entry::Separator, Entry::Separator, OTHER]),
            [false, true, false]
        );
    }

    #[test]
    fn a_separator_at_the_top_is_dropped() {
        assert_eq!(kinds(&[Entry::Separator, ITEM]), [false]);
    }

    #[test]
    fn a_separator_at_the_bottom_is_dropped() {
        assert_eq!(kinds(&[ITEM, Entry::Separator]), [false]);
    }

    #[test]
    fn a_menu_of_nothing_but_separators_comes_out_empty() {
        assert!(tidy(&[Entry::Separator, Entry::Separator]).is_empty());
    }

    #[test]
    fn a_separator_that_separates_two_things_stays() {
        assert_eq!(
            kinds(&[ITEM, Entry::Separator, OTHER]),
            [false, true, false]
        );
    }

    /// The reported case, straight from the Local menu.
    #[test]
    fn the_local_menu_has_no_double_separator() {
        let kinds = kinds(LOCAL);
        assert!(
            !kinds.windows(2).any(|pair| pair[0] && pair[1]),
            "two separators in a row survived"
        );
    }

    #[test]
    fn no_menu_of_the_application_starts_or_ends_with_a_separator() {
        for entries in [REPOSITORY, VIEW, LOCAL, REMOTE, BRANCH, QUERY, TOOLS, HELP] {
            let kinds = kinds(entries);
            assert_eq!(kinds.first(), Some(&false), "a menu began with a separator");
            assert_eq!(kinds.last(), Some(&false), "a menu ended with a separator");
        }
    }
}
