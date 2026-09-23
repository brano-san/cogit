//! The Blame window: one file at one commit, in a window of its own (#10).

use crate::child_window::{CLOSE, Item, Submenu};

/// Minimal on purpose: what applies to this one window, nothing of the main one.
pub const MENU: &[Submenu] = &[
    Submenu {
        title: "File",
        items: &[Item {
            action: CLOSE,
            label: "Close",
            accelerator: Some("CmdOrCtrl+W"),
        }],
    },
    Submenu {
        title: "View",
        items: &[
            Item {
                action: "refresh",
                label: "Refresh",
                accelerator: Some("F5"),
            },
            Item {
                action: "toggle-history",
                label: "History of Current Line",
                accelerator: None,
            },
        ],
    },
];

const SHORT_OID: usize = 7;

/// SmartGit's: `<file> - Blame of <path>@<short hash>`.
pub fn title(path: &str, oid: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    let short = oid.get(..SHORT_OID).unwrap_or(oid);
    format!("{name} - Blame of {path}@{short}")
}

/// In the URL rather than in shared state: the window rebuilds itself after a webview
/// reload (T2.5), and nothing the main window holds can be overwritten by it.
pub fn url(repo: u32, path: &str, oid: &str) -> String {
    format!(
        "blame.html?repo={repo}&path={}&rev={}",
        encode(path),
        encode(oid)
    )
}

/// Everything but the unreserved characters and `/` is percent-encoded, `+` included:
/// `URLSearchParams` would read a bare one as a space.
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/".contains(&byte) {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const OID: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn the_title_names_the_file_its_path_and_the_short_hash() {
        assert_eq!(
            title("src/lib/main.rs", OID),
            "main.rs - Blame of src/lib/main.rs@0123456"
        );
    }

    #[test]
    fn a_file_at_the_root_is_its_own_path() {
        assert_eq!(
            title("README.md", OID),
            "README.md - Blame of README.md@0123456"
        );
    }

    #[test]
    fn the_url_survives_any_character_a_path_can_hold() {
        assert_eq!(
            url(3, "a b/c&d#e+f%é.rs", "abc"),
            "blame.html?repo=3&path=a%20b/c%26d%23e%2Bf%25%C3%A9.rs&rev=abc"
        );
    }

    /// An action becomes a JavaScript string literal and a DOM event name's detail.
    #[test]
    fn menu_actions_are_plain_and_distinct() {
        let actions: Vec<&str> = MENU
            .iter()
            .flat_map(|submenu| submenu.items.iter().map(|item| item.action))
            .collect();
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
    fn the_window_can_be_closed_from_its_menu() {
        assert!(
            MENU.iter()
                .flat_map(|submenu| submenu.items)
                .any(|item| item.action == CLOSE && item.accelerator == Some("CmdOrCtrl+W"))
        );
    }
}
