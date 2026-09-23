//! The windows Cogit opens beside the main one: one file being compared, one conflict
//! being resolved, one file blamed.
//!
//! They are not small copies of the main window. They carry no application menu (there is
//! nothing in `Repository ▸ …` that applies to one file), they start dark rather than
//! white so a slow load does not flash a blank page, and they refuse to be made too small
//! to read a diff in (doc/12-risks.md, R-111). Closing one touches nothing the main window
//! owns (R-201).

use std::sync::atomic::{AtomicU32, Ordering};

use tauri::utils::config::Color;
use tauri::{WebviewUrl, WebviewWindowBuilder};

/// The label `tauri.conf.json` gives the one window that owns the application.
pub const MAIN: &str = "main";

/// `--c-bg-window` from the dark theme. A window that has not painted yet shows this
/// instead of the webview's default white.
const BACKGROUND: Color = Color(0x0f, 0x11, 0x15, 0xff);

/// Labels must be unique for the lifetime of the process. Counting the open windows is
/// not enough: close one of two and the next window is handed a label that is still in
/// use by the other, and the build fails.
static NEXT: AtomicU32 = AtomicU32::new(1);

#[must_use]
pub fn is_main(label: &str) -> bool {
    label == MAIN
}

pub struct Shape {
    pub width: f64,
    pub height: f64,
    pub min_width: f64,
    pub min_height: f64,
}

/// Blocks until the window exists, so never call it on the main thread (R-201).
pub fn open<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    kind: &str,
    url: String,
    title: String,
    shape: Shape,
) -> tauri::Result<()> {
    let label = format!("{kind}-{}", NEXT.fetch_add(1, Ordering::Relaxed));
    // Without a menu of its own the window inherits the application's. An empty one still
    // draws a blank bar, so it is taken off again before the window is first shown.
    let empty = tauri::menu::Menu::new(app)?;

    let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
        .title(title)
        .menu(empty)
        .visible(false)
        .background_color(BACKGROUND)
        .inner_size(shape.width, shape.height)
        .min_inner_size(shape.min_width, shape.min_height)
        .center()
        .build()?;
    window.remove_menu()?;
    window.show()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels_are_unique(taken: &[String]) -> bool {
        let mut seen = std::collections::HashSet::new();
        taken.iter().all(|label| seen.insert(label))
    }

    fn labels(kind: &str, count: usize) -> Vec<String> {
        (0..count)
            .map(|_| format!("{kind}-{}", NEXT.fetch_add(1, Ordering::Relaxed)))
            .collect()
    }

    #[test]
    fn two_windows_of_the_same_kind_never_share_a_label() {
        assert!(labels_are_unique(&labels("compare", 5)));
    }

    /// The old scheme numbered by how many windows were open, so closing one and opening
    /// another reused a live label.
    #[test]
    fn closing_one_and_opening_another_does_not_reuse_a_label() {
        let mut taken = labels("compare", 2);
        taken.remove(0);
        taken.extend(labels("compare", 1));
        assert!(labels_are_unique(&taken));
    }

    #[test]
    fn two_kinds_cannot_collide_either() {
        let mut taken = labels("compare", 2);
        taken.extend(labels("merge", 2));
        assert!(labels_are_unique(&taken));
    }

    /// Closing a diff window used to arm the shutdown watchdog of the whole app (#8).
    #[test]
    fn a_window_opened_here_is_never_the_main_one() {
        assert!(is_main(MAIN));
        for label in labels("compare", 1).iter().chain(&labels("blame", 1)) {
            assert!(!is_main(label), "{label}");
        }
    }

    fn granted_windows() -> Vec<String> {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        capability["windows"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|label| label.as_str().map(str::to_owned))
            .collect()
    }

    /// The first string literal after each call that opens a child window.
    fn opened_kinds() -> Vec<String> {
        let needle = concat!("child_window", "::open");
        let mut kinds = Vec::new();
        let mut pending = vec![std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src"
        ))];
        while let Some(dir) = pending.pop() {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    pending.push(path);
                    continue;
                }
                if path.extension().is_none_or(|ext| ext != "rs") {
                    continue;
                }
                let source = std::fs::read_to_string(&path).unwrap();
                for (at, _) in source.match_indices(needle) {
                    let rest = source[at + needle.len()..]
                        .trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '_');
                    if !rest.starts_with('(') {
                        continue;
                    }
                    let kind = rest.split('"').nth(1).unwrap_or_default();
                    kinds.push(kind.to_owned());
                }
            }
        }
        kinds.sort();
        kinds.dedup();
        kinds
    }

    /// A window no capability names cannot call a single command: its content never loads
    /// and neither `Esc` nor `Ctrl+W` can close it.
    #[test]
    fn every_kind_of_window_may_call_commands() {
        let granted = granted_windows();
        let kinds = opened_kinds();
        assert!(kinds.contains(&"compare".to_owned()), "{kinds:?}");

        let missing: Vec<&String> = kinds
            .iter()
            .filter(|kind| !granted.contains(&format!("{kind}-*")))
            .collect();
        assert!(
            missing.is_empty(),
            "add `<kind>-*` to capabilities/default.json for: {missing:?}"
        );
    }
}
