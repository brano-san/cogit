use std::collections::HashMap;
use std::sync::Mutex;

use tauri::menu::{
    CheckMenuItem, CheckMenuItemBuilder, Menu, MenuItem, MenuItemBuilder, PredefinedMenuItem,
    Submenu, SubmenuBuilder,
};
use tauri::{AppHandle, Manager as _, Runtime};

mod context;
pub use context::{ContextItem, ContextMenu, popup};

/// Item ids are the palette command ids: one place decides what an action is called and
/// when it is available, and both the menu and the palette read it.
/// SmartGit's own split: this repository's `.git/config`, and the user's own file.
const EDIT_CONFIG: &[Entry] = &[
    Entry::Item("edit-config-repository", "Repository…", None),
    Entry::Item("edit-config-user", "User…", None),
];

const REPOSITORY: &[Entry] = &[
    Entry::Item("open", "Open Repository…", Some("CmdOrCtrl+O")),
    Entry::Item("scan", "Scan Folder for Repositories…", None),
    Entry::Item("close", "Close Repository", Some("CmdOrCtrl+W")),
    Entry::Separator,
    Entry::Item("worktree-add", "Add Worktree…", None),
    Entry::Item("worktree-remove", "Remove Worktree…", None),
    Entry::Item("worktree-prune", "Prune Obsolete Worktrees…", None),
    Entry::Separator,
    Entry::Item("repo-settings", "Settings…", None),
    Entry::Nested("Edit Git Config", EDIT_CONFIG),
    Entry::Separator,
    Entry::Item("exit", "Exit", Some("Alt+X")),
];

/// After the platform's own clipboard items, which `build` puts first.
const EDIT: &[Entry] = &[Entry::Item("settings", "Preferences…", Some("CmdOrCtrl+,"))];

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
    Entry::Check("panel-worktrees", "Worktrees Panel", Some("CmdOrCtrl+7")),
    Entry::Separator,
    Entry::Check("overlap", "Commit Overlap Column", None),
    Entry::Check("avatars", "Author Avatars", None),
    Entry::Separator,
    Entry::Check("perspective-main", "Perspective: Main", None),
    Entry::Check("perspective-review", "Perspective: Review", None),
    Entry::Item("reset-layout", "Reset Perspective", None),
];

const SUBMODULE: &[Entry] = &[
    Entry::Item("submodule-init", "Initialise", None),
    Entry::Item("submodule-sync", "Synchronise", None),
    Entry::Item("submodule-reset", "Reset…", None),
    Entry::Separator,
    Entry::Item("submodule-add", "Add…", None),
    Entry::Separator,
    Entry::Item("submodule-deactivate", "Deactivate…", None),
    Entry::Item("submodule-deinit", "Deinit…", None),
    Entry::Item("submodule-unregister", "Unregister…", None),
];

const SUBTREE: &[Entry] = &[
    Entry::Item("subtree-add", "Add…", None),
    Entry::Separator,
    Entry::Item("subtree-merge", "Merge…", None),
    Entry::Item("subtree-split", "Split…", None),
    Entry::Item("subtree-reset", "Reset…", None),
    Entry::Separator,
    Entry::Item("subtree-push", "Push…", None),
];

const LFS: &[Entry] = &[
    Entry::Item("lfs-install", "Install", None),
    Entry::Item("lfs-track", "Track…", None),
    Entry::Separator,
    Entry::Item("lfs-lock", "Lock", None),
    Entry::Item("lfs-unlock", "Unlock", None),
    Entry::Separator,
    Entry::Item("lfs-prune", "Prune…", None),
];

const REMOTE: &[Entry] = &[
    Entry::Item("fetch", "Fetch", Some("CmdOrCtrl+Shift+F")),
    Entry::Item("fetch-all", "Fetch All", Some("CmdOrCtrl+Alt+Shift+F")),
    Entry::Item("pull", "Pull", Some("CmdOrCtrl+Shift+U")),
    Entry::Item("push", "Push", Some("CmdOrCtrl+Shift+O")),
    Entry::Item("synchronize", "Synchronise", Some("CmdOrCtrl+Shift+S")),
    Entry::Separator,
    Entry::Nested("Submodule", SUBMODULE),
    Entry::Nested("Subtree", SUBTREE),
    Entry::Nested("LFS", LFS),
    Entry::Separator,
    Entry::Item("pr", "Create Pull Request", None),
];

const LOCAL: &[Entry] = &[
    Entry::Item("commit", "Commit…", Some("CmdOrCtrl+Enter")),
    Entry::Item("stage", "Stage", Some("CmdOrCtrl+T")),
    Entry::Item("unstage", "Unstage", Some("CmdOrCtrl+Shift+T")),
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
    Entry::Item(
        "rebase-i",
        "Rebase Commits After This One…",
        Some("CmdOrCtrl+Shift+R"),
    ),
    Entry::Item("split-off", "Split Off Files…", None),
    Entry::Item("rollback", "Roll Back Tree To This Commit", None),
];

const BRANCH: &[Entry] = &[
    Entry::Item("branch", "New Branch…", Some("F7")),
    Entry::Item("tag", "Create Tag", Some("Shift+F7")),
];

const QUERY: &[Entry] = &[
    Entry::Item("find", "Find Object…", Some("CmdOrCtrl+P")),
    Entry::Item("palette", "Find Command…", Some("CmdOrCtrl+Shift+P")),
    Entry::Separator,
    Entry::Item("blame", "Blame This File", Some("CmdOrCtrl+Shift+L")),
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
    ("Edit", EDIT),
    ("View", VIEW),
    ("Remote", REMOTE),
    ("Local", LOCAL),
    ("Branch", BRANCH),
    ("Query", QUERY),
    ("Tools", TOOLS),
    ("Help", HELP),
];

/// Keyed commands with no place on the bar (#43): the window still claims the key, or
/// WebView2 would take F5 as "reload the page", and the keymap editor still lists them.
const OFF_THE_BAR: &[(&str, &[Entry])] = &[
    (
        "Repository",
        &[Entry::Item("refresh", "Refresh", Some("F5"))],
    ),
    (
        "Edit",
        &[Entry::Item(
            "copy-sha",
            "Copy the Commit SHA",
            Some("CmdOrCtrl+Shift+Y"),
        )],
    ),
    (
        "Local",
        &[
            Entry::Item(
                "commit-amend",
                "Commit with Amend",
                Some("CmdOrCtrl+Shift+Enter"),
            ),
            Entry::Item("commit-message", "Commit Message", Some("CmdOrCtrl+K")),
        ],
    ),
];

fn keyed(section: &str, entries: &'static [Entry]) -> Vec<&'static Entry> {
    let mut all = leaves(entries);
    for (owner, extra) in OFF_THE_BAR {
        if *owner == section {
            all.extend(leaves(extra));
        }
    }
    all
}

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
        for entry in keyed(section, entries) {
            let (id, label, accelerator) = match entry {
                Entry::Item(id, label, keys) | Entry::Check(id, label, keys) => (id, label, keys),
                Entry::Separator | Entry::Nested(..) => continue,
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
    for (section, entries) in SECTIONS {
        for entry in keyed(section, entries) {
            if let Entry::Item(id, _, keys) | Entry::Check(id, _, keys) = entry {
                rows.push((*id, *keys));
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
    /// A submenu inside a menu; its items are commands like any other.
    Nested(&'static str, &'static [Entry]),
}

/// Items and checks in menu order, submenus opened up: what the keymap lists.
fn leaves(entries: &'static [Entry]) -> Vec<&'static Entry> {
    entries
        .iter()
        .flat_map(|entry| match entry {
            Entry::Nested(_, inner) => leaves(inner),
            Entry::Separator => Vec::new(),
            item => vec![item],
        })
        .collect()
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
    fill(
        app,
        SubmenuBuilder::new(app, title),
        entries,
        overrides,
        collected,
    )?
    .build()
}

/// Appends `entries` to a menu that may already hold the platform's own items.
fn fill<'m, R: Runtime>(
    app: &'m AppHandle<R>,
    mut builder: SubmenuBuilder<'m, R, AppHandle<R>>,
    entries: &[Entry],
    overrides: &HashMap<String, String>,
    collected: &mut Collected<R>,
) -> tauri::Result<SubmenuBuilder<'m, R, AppHandle<R>>> {
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
            Entry::Nested(title, inner) => {
                let nested = submenu(app, title, inner, overrides, collected)?;
                builder = builder.item(&nested);
            }
        }
    }
    Ok(builder)
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
    let clipboard = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .separator();
    let edit = fill(app, clipboard, EDIT, overrides, &mut collected)?.build()?;
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
///
/// The main window gets it, not the app: `App::set_menu` also hands it to every window
/// without a menu, and Compare and Merge dropped theirs (`child_window::open`).
pub fn rebuild<R: Runtime>(
    app: &AppHandle<R>,
    keymap: &Keymap,
    items: &MenuItems<R>,
    overrides: HashMap<String, String>,
) -> tauri::Result<()> {
    keymap.set(overrides);
    let (menu, collected) = build(app, &keymap.snapshot())?;
    match app.get_webview_window(crate::child_window::MAIN) {
        // macOS has one bar for the whole app.
        Some(main) if cfg!(not(target_os = "macos")) => {
            main.set_menu(menu)?;
        }
        _ => {
            app.set_menu(menu)?;
        }
    }
    items.replace(collected);
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
        for entries in [
            REPOSITORY, EDIT, VIEW, LOCAL, REMOTE, BRANCH, QUERY, TOOLS, HELP, SUBMODULE, SUBTREE,
            LFS,
        ] {
            let kinds = kinds(entries);
            assert_eq!(kinds.first(), Some(&false), "a menu began with a separator");
            assert_eq!(kinds.last(), Some(&false), "a menu ended with a separator");
        }
    }
}

#[cfg(test)]
mod nested_tests {
    use super::*;

    /// Repository ▸ Edit Git Config ▸ Repository / User: a submenu's items are commands
    /// like any other, so the keymap and the window accelerators must see them.
    #[test]
    fn items_inside_a_nested_menu_are_in_the_keymap() {
        let ids: Vec<&str> = default_keymap_pairs()
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert!(ids.contains(&"edit-config-repository"), "{ids:?}");
        assert!(ids.contains(&"edit-config-user"), "{ids:?}");
    }

    /// #43: Refresh left the bar, F5 stayed the window's.
    #[test]
    fn refresh_is_off_the_bar_but_f5_still_belongs_to_the_window() {
        let listed = leaves(REPOSITORY)
            .into_iter()
            .any(|entry| matches!(entry, Entry::Item("refresh", ..)));
        assert!(!listed, "Repository ▸ Refresh is still in the menu");
        assert!(default_keymap_pairs().contains(&("refresh", Some("F5"))));
        let row = default_keymap()
            .into_iter()
            .find(|row| row.id == "refresh")
            .expect("listed in the keymap editor");
        assert_eq!(row.section, "Repository");
    }

    /// 11 §4: Commit, Commit with Amend and the message field answer anywhere in the window.
    #[test]
    fn the_commit_keys_belong_to_the_window() {
        let pairs = default_keymap_pairs();
        assert!(pairs.contains(&("commit", Some("CmdOrCtrl+Enter"))));
        assert!(pairs.contains(&("commit-amend", Some("CmdOrCtrl+Shift+Enter"))));
        assert!(pairs.contains(&("commit-message", Some("CmdOrCtrl+K"))));
        let claimed = crate::accelerators::table(pairs, &HashMap::new());
        for keys in ["CmdOrCtrl+Enter", "CmdOrCtrl+Shift+Enter", "CmdOrCtrl+K"] {
            let chord = crate::accelerators::parse(keys).expect("parses");
            assert!(claimed.contains_key(&chord), "{keys}");
        }
        let row = default_keymap()
            .into_iter()
            .find(|row| row.id == "commit-amend")
            .expect("listed");
        assert_eq!(row.section, "Local");
    }

    /// 11 promised these and the toolbar and file menus showed them, but no item had them.
    #[test]
    fn the_registry_keys_are_the_window_s() {
        let pairs = default_keymap_pairs();
        for pair in [
            ("stage", Some("CmdOrCtrl+T")),
            ("unstage", Some("CmdOrCtrl+Shift+T")),
            ("fetch-all", Some("CmdOrCtrl+Alt+Shift+F")),
            ("blame", Some("CmdOrCtrl+Shift+L")),
            ("copy-sha", Some("CmdOrCtrl+Shift+Y")),
            ("branch", Some("F7")),
            ("tag", Some("Shift+F7")),
            ("rebase-i", Some("CmdOrCtrl+Shift+R")),
        ] {
            assert!(pairs.contains(&pair), "{pair:?}");
        }
    }

    #[test]
    fn the_keymap_editor_lists_them_under_their_menu() {
        let row = default_keymap()
            .into_iter()
            .find(|row| row.id == "edit-config-user")
            .expect("listed");
        assert_eq!(row.section, "Repository");
    }
}

#[cfg(test)]
mod remote_tests {
    use super::*;

    fn outline(entries: &[Entry]) -> Vec<String> {
        tidy(entries)
            .into_iter()
            .map(|entry| match entry {
                Entry::Item(_, label, _) | Entry::Check(_, label, _) => (*label).to_owned(),
                Entry::Separator => "-".to_owned(),
                Entry::Nested(title, inner) => format!("{title} ▸ {:?}", outline(inner)),
            })
            .collect()
    }

    fn nested(title: &str) -> &'static [Entry] {
        REMOTE
            .iter()
            .find_map(|entry| match entry {
                Entry::Nested(name, inner) if *name == title => Some(*inner),
                _ => None,
            })
            .unwrap_or_else(|| panic!("Remote has no {title} submenu"))
    }

    #[test]
    fn the_submodule_menu_is_in_the_asked_order() {
        assert_eq!(
            outline(nested("Submodule")),
            [
                "Initialise",
                "Synchronise",
                "Reset…",
                "-",
                "Add…",
                "-",
                "Deactivate…",
                "Deinit…",
                "Unregister…"
            ]
        );
    }

    #[test]
    fn the_subtree_menu_is_in_the_asked_order() {
        assert_eq!(
            outline(nested("Subtree")),
            ["Add…", "-", "Merge…", "Split…", "Reset…", "-", "Push…"]
        );
    }

    /// #46: next to Submodule and Subtree, where the other extensions of a remote live.
    #[test]
    fn lfs_sits_beside_submodule_and_subtree() {
        assert_eq!(
            outline(nested("LFS")),
            ["Install", "Track…", "-", "Lock", "Unlock", "-", "Prune…"]
        );
        let titles: Vec<&str> = REMOTE
            .iter()
            .filter_map(|entry| match entry {
                Entry::Nested(title, _) => Some(*title),
                _ => None,
            })
            .collect();
        assert_eq!(titles, ["Submodule", "Subtree", "LFS"]);
    }

    #[test]
    fn synchronize_is_a_remote_command_with_the_sync_key() {
        assert!(default_keymap_pairs().contains(&("synchronize", Some("CmdOrCtrl+Shift+S"))));
        let row = default_keymap()
            .into_iter()
            .find(|row| row.id == "synchronize")
            .expect("listed");
        assert_eq!(row.section, "Remote");
    }

    /// Enabled by id: two items sharing one would be enabled and disabled together.
    #[test]
    fn no_two_items_share_an_id() {
        let mut ids: Vec<&str> = default_keymap_pairs()
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
    }

    /// #42: Cogit's own settings moved to `Edit ▸ Preferences…`, as in SmartGit.
    #[test]
    fn settings_in_the_repository_menu_are_the_repository_s() {
        let repository = leaves(REPOSITORY);
        assert!(
            repository
                .iter()
                .any(|entry| matches!(entry, Entry::Item("repo-settings", "Settings…", _)))
        );
        assert!(
            !repository
                .iter()
                .any(|entry| matches!(entry, Entry::Item("settings", ..)))
        );
        assert!(default_keymap_pairs().contains(&("settings", Some("CmdOrCtrl+,"))));
        let row = default_keymap()
            .into_iter()
            .find(|row| row.id == "settings")
            .expect("listed");
        assert_eq!(
            (row.section.as_str(), row.label.as_str()),
            ("Edit", "Preferences…")
        );
    }
}

#[cfg(test)]
mod edit_tests {
    use super::*;

    /// The toolbar is set up in Preferences ▸ Toolbar only; Edit keeps Preferences….
    #[test]
    fn configure_toolbar_is_gone_from_the_edit_menu() {
        assert!(
            !default_keymap()
                .iter()
                .any(|row| row.id == "configure-toolbar")
        );
        assert!(
            !EDIT
                .iter()
                .any(|entry| matches!(entry, Entry::Item("configure-toolbar", ..)))
        );
        assert!(
            EDIT.iter()
                .any(|entry| matches!(entry, Entry::Item("settings", "Preferences…", _)))
        );
    }
}
