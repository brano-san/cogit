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

/// One entry of a child window's own menu bar. [`CLOSE`] is answered in Rust; any other
/// action reaches the page as a `cogit-menu` DOM event whose `detail` is the action.
pub struct Item {
    pub action: &'static str,
    pub label: &'static str,
    pub accelerator: Option<&'static str>,
}

pub struct Submenu {
    pub title: &'static str,
    pub items: &'static [Item],
}

pub const CLOSE: &str = "close";

/// Every menu id of a child window starts with it: menu events reach every handler in the
/// app, and the main window's must be able to tell them apart.
const MENU_PREFIX: &str = "child:";

/// A window with no menu bar at all. Blocks until the window exists, so never call it on
/// the main thread (R-201).
pub fn open<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    kind: &str,
    url: String,
    title: String,
    shape: Shape,
) -> tauri::Result<()> {
    open_with_menu(app, kind, url, title, shape, &[])
}

/// Same as [`open`], with a menu bar of the window's own.
pub fn open_with_menu<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    kind: &str,
    url: String,
    title: String,
    shape: Shape,
    menu: &[Submenu],
) -> tauri::Result<()> {
    let label = format!("{kind}-{}", NEXT.fetch_add(1, Ordering::Relaxed));
    // Without a menu of its own the window inherits the application's. An empty one still
    // draws a blank bar, so it is taken off again before the window is first shown.
    let bar = build_menu(app, &label, menu)?;

    let mut builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
        .title(title)
        .menu(bar)
        .visible(false)
        .background_color(BACKGROUND)
        .inner_size(shape.width, shape.height)
        .min_inner_size(shape.min_width, shape.min_height)
        .center();
    // WebView2 refuses a second environment on the same profile with other arguments.
    if let Some(args) = browser_args(&app.config().app.windows) {
        builder = builder.additional_browser_args(&args);
    }
    let window = builder.build()?;
    if menu.is_empty() {
        window.remove_menu()?;
    }
    #[cfg(windows)]
    crate::webview2::install_child_accelerators(&window, accelerator_table(window.label(), menu));
    window.show()
}

fn build_menu<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    label: &str,
    menu: &[Submenu],
) -> tauri::Result<tauri::menu::Menu<R>> {
    let bar = tauri::menu::Menu::new(app)?;
    for submenu in menu {
        let entries = tauri::menu::Submenu::new(app, submenu.title, true)?;
        for item in submenu.items {
            entries.append(&tauri::menu::MenuItem::with_id(
                app,
                menu_id(label, item.action),
                item.label,
                true,
                item.accelerator,
            )?)?;
        }
        bar.append(&entries)?;
    }
    Ok(bar)
}

/// The keys of this window's own menu, claimed ahead of WebView2 the way the main window's
/// are: while the page has focus the menu's accelerators never fire otherwise.
#[must_use]
pub fn accelerator_table(
    label: &str,
    menu: &[Submenu],
) -> std::collections::HashMap<crate::accelerators::Chord, String> {
    let ids: Vec<(String, Option<&str>)> = menu
        .iter()
        .flat_map(|submenu| submenu.items.iter())
        .map(|item| (menu_id(label, item.action), item.accelerator))
        .collect();
    crate::accelerators::table(
        ids.iter().map(|(id, keys)| (id.as_str(), *keys)),
        &std::collections::HashMap::new(),
    )
}

fn menu_id(label: &str, action: &str) -> String {
    format!("{MENU_PREFIX}{label}:{action}")
}

fn parse_menu_id(id: &str) -> Option<(&str, &str)> {
    id.strip_prefix(MENU_PREFIX)?.rsplit_once(':')
}

/// Actions are ids from our own tables (`[a-z-]`), so the debug form is valid JavaScript.
fn menu_script(action: &str) -> String {
    format!(r#"window.dispatchEvent(new CustomEvent("cogit-menu", {{ detail: {action:?} }}))"#)
}

/// Runs a child window's menu item; `false` when the id belongs to the main window.
pub fn on_menu<R: tauri::Runtime>(app: &tauri::AppHandle<R>, id: &str) -> bool {
    use tauri::Manager as _;

    let Some((label, action)) = parse_menu_id(id) else {
        return false;
    };
    let Some(window) = app.get_webview_window(label) else {
        return true;
    };
    let done = if action == CLOSE {
        window.close()
    } else {
        window.eval(menu_script(action))
    };
    if let Err(err) = done {
        tracing::warn!(error = %err, label, action, "a child window's menu item failed");
    }
    true
}

/// The main window's browser arguments, which every other webview has to repeat.
fn browser_args(windows: &[tauri::utils::config::WindowConfig]) -> Option<String> {
    windows
        .iter()
        .find(|window| is_main(&window.label))
        .and_then(|window| window.additional_browser_args.clone())
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

    // The keys of a child window's own menu were claimed nowhere: WebView2 took them while
    // the page had focus, so Ctrl+2, Alt+Left or F1 in Investigate did nothing.
    #[test]
    fn a_child_windows_menu_keys_are_claimed_for_that_window() {
        let table = accelerator_table("investigate-3", crate::commands::investigate::MENU);
        let claimed = |keys: &str| {
            let chord = crate::accelerators::parse(keys).expect("parses");
            table.get(&chord).cloned()
        };
        assert_eq!(
            claimed("Alt+Left").as_deref(),
            Some("child:investigate-3:back")
        );
        assert_eq!(
            claimed("CmdOrCtrl+2").as_deref(),
            Some("child:investigate-3:perspective-diff")
        );
        assert_eq!(
            claimed("F5").as_deref(),
            Some("child:investigate-3:refresh")
        );
        assert_eq!(
            claimed("CmdOrCtrl+W").as_deref(),
            Some("child:investigate-3:close")
        );
    }

    #[test]
    fn a_window_without_a_menu_claims_nothing() {
        assert!(accelerator_table("merge-1", &[]).is_empty());
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

    #[test]
    fn a_menu_id_names_its_window_and_action() {
        let id = menu_id("blame-4", "refresh");
        assert_eq!(parse_menu_id(&id), Some(("blame-4", "refresh")));
    }

    /// Menu events reach every handler in the app; the main window's must not run a child
    /// window's item as a palette command.
    #[test]
    fn an_application_menu_id_is_not_a_child_one() {
        assert_eq!(parse_menu_id("refresh"), None);
        assert_eq!(parse_menu_id("copy-diagnostics"), None);
    }

    #[test]
    fn a_menu_action_reaches_the_page_as_a_dom_event() {
        assert_eq!(
            menu_script("toggle-history"),
            r#"window.dispatchEvent(new CustomEvent("cogit-menu", { detail: "toggle-history" }))"#
        );
    }

    fn windows_of(config: &str) -> Vec<tauri::utils::config::WindowConfig> {
        let value: serde_json::Value = serde_json::from_str(config).unwrap();
        serde_json::from_value(value["app"]["windows"].clone()).unwrap()
    }

    /// The benchmark's config opens a debugging port on the main window; a child window
    /// without the same arguments failed to get a webview at all.
    #[test]
    fn a_child_repeats_the_main_windows_browser_arguments() {
        let args = browser_args(&windows_of(include_str!("../tauri.bench.conf.json")));
        assert!(
            args.is_some_and(|args| args.contains("--remote-debugging-port")),
            "the child must join the same WebView2 environment"
        );
    }

    #[test]
    fn a_main_window_without_arguments_leaves_the_defaults_alone() {
        assert_eq!(
            browser_args(&windows_of(include_str!("../tauri.conf.json"))),
            None
        );
    }

    /// An overlay config replaces the `windows` array whole (RFC 7396). The debug one set
    /// only its browser arguments, and the main window came up with no size, no minimum,
    /// no theme and no background.
    #[test]
    fn an_overlay_repeats_every_field_of_the_main_window() {
        let base: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let fields = base["app"]["windows"][0].as_object().unwrap();
        for (name, overlay) in [
            ("debug", include_str!("../tauri.debug.conf.json")),
            ("bench", include_str!("../tauri.bench.conf.json")),
        ] {
            let overlay: serde_json::Value = serde_json::from_str(overlay).unwrap();
            let window = overlay["app"]["windows"][0].as_object().unwrap();
            let missing: Vec<&String> = fields
                .keys()
                .filter(|key| !window.contains_key(*key))
                .collect();
            assert!(missing.is_empty(), "the {name} overlay drops {missing:?}");
        }
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
