//! The Investigate window: the file's log, blame with origins, origin candidates (#15).

use crate::child_window::{CLOSE, Item, Shape, Submenu};

pub const SHAPE: Shape = Shape {
    width: 1280.0,
    height: 860.0,
    min_width: 820.0,
    min_height: 560.0,
};

const fn item(
    action: &'static str,
    label: &'static str,
    accelerator: Option<&'static str>,
) -> Item {
    Item {
        action,
        label,
        accelerator,
    }
}

/// File, Edit, View, Go To, Window, Help — DeepGit's bar. Every action but Close reaches
/// the page as a `cogit-menu` event (`lib/investigate/menu.ts`).
pub const MENU: &[Submenu] = &[
    Submenu {
        title: "File",
        items: &[item(CLOSE, "Close", Some("CmdOrCtrl+W"))],
    },
    Submenu {
        title: "Edit",
        items: &[
            item("copy-line", "Copy Line", None),
            item(
                "copy-commit-id",
                "Copy Commit ID",
                Some("CmdOrCtrl+Shift+C"),
            ),
            item("copy-path", "Copy File Path", None),
        ],
    },
    Submenu {
        title: "View",
        items: &[
            item("follow-renames", "Follow Renames (on/off)", None),
            item(
                "ignore-whitespace",
                "Ignore Whitespace Changes (on/off)",
                None,
            ),
            item("refresh", "Refresh", Some("F5")),
        ],
    },
    Submenu {
        title: "Go To",
        items: &[
            item("back", "Back", Some("Alt+Left")),
            item("forward", "Forward", Some("Alt+Right")),
            item("go-deeper", "Go Deeper", Some("CmdOrCtrl+D")),
            item("close-card", "Hide Origin Card", None),
            item("previous-change", "Previous Change", Some("Shift+F6")),
            item("next-change", "Next Change", Some("F6")),
            item("newer-version", "Newer Version", Some("Alt+Up")),
            item("older-version", "Older Version", Some("Alt+Down")),
        ],
    },
    Submenu {
        title: "Window",
        items: &[
            item("perspective-log", "Log", Some("CmdOrCtrl+1")),
            item("perspective-diff", "Diff", Some("CmdOrCtrl+2")),
            item("perspective-blame", "Blame", Some("CmdOrCtrl+3")),
            item(
                "perspective-blame-origins",
                "Blame+Origins",
                Some("CmdOrCtrl+4"),
            ),
            item("perspective-origins", "Origins", Some("CmdOrCtrl+5")),
        ],
    },
    Submenu {
        title: "Help",
        items: &[item("help", "How Investigate Works", Some("F1"))],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> impl Iterator<Item = &'static Item> {
        MENU.iter().flat_map(|submenu| submenu.items.iter())
    }

    #[test]
    fn the_bar_has_the_six_menus_the_window_promises() {
        let titles: Vec<&str> = MENU.iter().map(|submenu| submenu.title).collect();
        assert_eq!(titles, ["File", "Edit", "View", "Go To", "Window", "Help"]);
    }

    /// An action becomes a JavaScript string literal and a DOM event's detail.
    #[test]
    fn actions_are_plain_and_distinct() {
        let actions: Vec<&str> = items().map(|item| item.action).collect();
        assert!(
            actions
                .iter()
                .all(|action| action.chars().all(|c| c.is_ascii_lowercase() || c == '-'))
        );
        let mut distinct = actions.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), actions.len());
    }

    #[test]
    fn no_two_items_share_a_shortcut() {
        let mut keys: Vec<&str> = items().filter_map(|item| item.accelerator).collect();
        let all = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), all);
    }

    #[test]
    fn the_window_closes_from_its_menu_with_ctrl_w() {
        assert!(
            items().any(|item| item.action == CLOSE && item.accelerator == Some("CmdOrCtrl+W"))
        );
    }
}
