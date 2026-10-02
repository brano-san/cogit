//! The menu bar as data, for the renderer that draws it in the page.
//!
//! Built from the same tables as the native bar, so the two renderers cannot drift: a
//! command added to `SECTIONS` appears in both, with the keys of the effective keymap.

use std::collections::HashMap;

use super::{Entry, SECTIONS, accelerator, tidy};

/// One row of a menu, at any depth. Ids are the palette command ids; a click travels back
/// through `menu_command`, the path a native menu event takes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MenuNode {
    pub id: String,
    pub label: String,
    pub separator: bool,
    /// As the keymap writes it (`CmdOrCtrl+Shift+P`); the page formats it for display.
    pub accelerator: Option<String>,
    pub enabled: bool,
    /// `Some` for a toggle, with its tick.
    pub checked: Option<bool>,
    pub children: Vec<MenuNode>,
}

/// What the frontend last pushed through `set_menu_state`.
#[derive(Clone, Copy, Default)]
pub struct Flags<'a> {
    pub disabled: &'a [String],
    pub checked: &'a [String],
}

impl MenuNode {
    fn row(id: &str, label: &str, accelerator: Option<&str>) -> Self {
        Self {
            id: id.to_owned(),
            label: label.to_owned(),
            separator: false,
            accelerator: accelerator.map(str::to_owned),
            enabled: true,
            checked: None,
            children: Vec::new(),
        }
    }

    fn line() -> Self {
        Self {
            separator: true,
            ..Self::row("", "", None)
        }
    }

    fn menu(id_prefix: &str, title: &str, children: Vec<Self>) -> Self {
        Self {
            children,
            ..Self::row(&format!("{id_prefix}:{}", slug(title)), title, None)
        }
    }
}

fn slug(title: &str) -> String {
    title.to_lowercase().replace(' ', "-")
}

fn nodes(
    entries: &[Entry],
    overrides: &HashMap<String, String>,
    flags: Flags<'_>,
) -> Vec<MenuNode> {
    tidy(entries)
        .into_iter()
        .map(|entry| match entry {
            Entry::Separator => MenuNode::line(),
            Entry::Item(id, label, fallback) | Entry::Check(id, label, fallback) => {
                let keys = accelerator(overrides, id, fallback.as_ref());
                let mut node = MenuNode::row(id, label, keys);
                node.enabled = !flags.disabled.iter().any(|off| off == id);
                if matches!(entry, Entry::Check(..)) {
                    node.checked = Some(flags.checked.iter().any(|on| on == id));
                }
                node
            }
            Entry::Nested(title, inner) => {
                MenuNode::menu("submenu", title, nodes(inner, overrides, flags))
            }
        })
        .collect()
}

/// The platform's own clipboard rows, which the webview answers itself. The keys are shown,
/// never bound: the page already owns them.
fn clipboard() -> Vec<MenuNode> {
    let redo = if cfg!(windows) {
        "CmdOrCtrl+Y"
    } else {
        "CmdOrCtrl+Shift+Z"
    };
    vec![
        MenuNode::row("edit-undo", "Undo", Some("CmdOrCtrl+Z")),
        MenuNode::row("edit-redo", "Redo", Some(redo)),
        MenuNode::line(),
        MenuNode::row("edit-cut", "Cut", Some("CmdOrCtrl+X")),
        MenuNode::row("edit-copy", "Copy", Some("CmdOrCtrl+C")),
        MenuNode::row("edit-paste", "Paste", Some("CmdOrCtrl+V")),
        MenuNode::row("select-all", "Select All", Some("CmdOrCtrl+A")),
        MenuNode::line(),
    ]
}

fn window_menu() -> Vec<MenuNode> {
    vec![
        MenuNode::row("window-minimize", "Minimize", None),
        MenuNode::row("window-maximize", "Maximize", None),
        MenuNode::line(),
        MenuNode::row("reset-window-position", "Reset Window Position", None),
        MenuNode::line(),
        MenuNode::row("window-close", "Close", None),
    ]
}

/// The main window's bar, in the order `build` gives the native one.
#[must_use]
pub fn app_menu(overrides: &HashMap<String, String>, flags: Flags<'_>) -> Vec<MenuNode> {
    let mut bar = Vec::new();
    for (title, entries) in SECTIONS {
        if *title == "Help" {
            bar.push(MenuNode::menu("menu", "Window", window_menu()));
        }
        let mut rows = if *title == "Edit" {
            clipboard()
        } else {
            Vec::new()
        };
        rows.extend(nodes(entries, overrides, flags));
        bar.push(MenuNode::menu("menu", title, rows));
    }
    bar
}

/// A child window's own bar; its ids are the ones the native bar would raise.
#[must_use]
pub fn child_menu(label: &str, menu: &[crate::child_window::Submenu]) -> Vec<MenuNode> {
    menu.iter()
        .map(|submenu| {
            let rows = submenu
                .items
                .iter()
                .map(|item| {
                    MenuNode::row(
                        &crate::child_window::menu_id(label, item.action),
                        item.label,
                        item.accelerator,
                    )
                })
                .collect();
            MenuNode::menu("menu", submenu.title, rows)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar() -> Vec<MenuNode> {
        app_menu(&HashMap::new(), Flags::default())
    }

    fn walk<'a>(rows: &'a [MenuNode], out: &mut Vec<&'a MenuNode>) {
        for row in rows {
            out.push(row);
            walk(&row.children, out);
        }
    }

    fn find<'a>(rows: &'a [MenuNode], id: &str) -> &'a MenuNode {
        let mut flat = Vec::new();
        walk(rows, &mut flat);
        flat.into_iter()
            .find(|row| row.id == id)
            .unwrap_or_else(|| panic!("no row {id}"))
    }

    #[test]
    fn the_titles_follow_the_native_bar() {
        let titles: Vec<String> = bar().into_iter().map(|menu| menu.label).collect();
        assert_eq!(
            titles,
            [
                "Repository",
                "Edit",
                "View",
                "Remote",
                "Local",
                "Branch",
                "Query",
                "Tools",
                "Window",
                "Help"
            ]
        );
    }

    #[test]
    fn every_command_of_the_native_bar_is_in_the_model() {
        let bar = bar();
        for (id, _) in crate::menu::default_keymap_pairs() {
            let off_the_bar = matches!(
                id,
                "refresh" | "copy-sha" | "commit-amend" | "commit-window" | "commit-message"
            );
            if !off_the_bar {
                find(&bar, id);
            }
        }
    }

    #[test]
    fn ids_are_unique() {
        let bar = bar();
        let mut flat = Vec::new();
        walk(&bar, &mut flat);
        let mut ids: Vec<&str> = flat
            .iter()
            .filter(|row| !row.separator)
            .map(|row| row.id.as_str())
            .collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
    }

    #[test]
    fn no_menu_has_a_doubled_leading_or_trailing_separator() {
        fn check(rows: &[MenuNode]) {
            let lines: Vec<bool> = rows.iter().map(|row| row.separator).collect();
            assert_ne!(lines.first(), Some(&true));
            assert_ne!(lines.last(), Some(&true));
            assert!(!lines.windows(2).any(|pair| pair[0] && pair[1]));
            for row in rows {
                check(&row.children);
            }
        }
        for menu in bar() {
            check(&menu.children);
        }
    }

    #[test]
    fn accelerators_come_from_the_effective_keymap() {
        let bar = bar();
        assert_eq!(
            find(&bar, "open").accelerator.as_deref(),
            Some("CmdOrCtrl+O")
        );
        let mut overrides = HashMap::new();
        overrides.insert("open".to_owned(), "CmdOrCtrl+Shift+O".to_owned());
        overrides.insert("close".to_owned(), String::new());
        let custom = app_menu(&overrides, Flags::default());
        assert_eq!(
            find(&custom, "open").accelerator.as_deref(),
            Some("CmdOrCtrl+Shift+O")
        );
        assert_eq!(find(&custom, "close").accelerator, None);
    }

    #[test]
    fn state_reaches_enabled_and_checked() {
        let disabled = vec!["push".to_owned()];
        let checked = vec!["output".to_owned()];
        let flags = Flags {
            disabled: &disabled,
            checked: &checked,
        };
        let bar = app_menu(&HashMap::new(), flags);
        assert!(!find(&bar, "push").enabled);
        assert!(find(&bar, "fetch").enabled);
        assert_eq!(find(&bar, "output").checked, Some(true));
        assert_eq!(find(&bar, "overlap").checked, Some(false));
        assert_eq!(find(&bar, "fetch").checked, None);
    }

    #[test]
    fn nested_menus_keep_their_items() {
        let bar = bar();
        let lfs = find(&bar, "submenu:lfs");
        assert!(lfs.children.iter().any(|row| row.id == "lfs-install"));
    }

    #[test]
    fn edit_starts_with_the_clipboard() {
        let bar = bar();
        let edit = find(&bar, "menu:edit");
        assert_eq!(edit.children[0].id, "edit-undo");
        assert!(edit.children.iter().any(|row| row.id == "settings"));
    }

    #[test]
    fn a_child_windows_ids_name_the_window() {
        let menu = child_menu("investigate-3", crate::investigate_window::MENU);
        let close = find(&menu, "child:investigate-3:close");
        assert_eq!(close.accelerator.as_deref(), Some("CmdOrCtrl+W"));
    }
}
