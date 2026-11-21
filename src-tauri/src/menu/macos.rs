use std::collections::HashMap;

use tauri::menu::{Submenu, SubmenuBuilder};
use tauri::{AppHandle, Runtime};

use super::{Collected, Entry, fill};

const APP_ITEMS: &[Entry] = &[
    Entry::Item("about", "About Cogit", None),
    Entry::Item("check-updates", "Check for Updates…", None),
    Entry::Separator,
    Entry::Item("settings", "Preferences…", Some("CmdOrCtrl+,")),
];
const QUIT: Entry = Entry::Item("exit", "Quit Cogit", Some("CmdOrCtrl+Q"));
pub(super) const EDIT_ID: &str = "cogit-edit";
pub(super) const APP_ID: &str = "cogit-app";

fn app_entry(id: &str) -> Option<&'static Entry> {
    APP_ITEMS
        .iter()
        .chain([&QUIT])
        .find(|entry| matches!(entry, Entry::Item(key, ..) if *key == id))
}

pub(super) fn command_location<'a>(
    mac: bool,
    section: &'a str,
    id: &str,
    label: &'a str,
    accelerator: Option<&'static str>,
) -> (&'a str, &'a str, Option<&'static str>) {
    if mac {
        if let Some(Entry::Item(_, name, keys)) = app_entry(id) {
            return ("Cogit", name, *keys);
        }
        if id == "close" {
            return (section, label, None);
        }
    }
    (section, label, accelerator)
}

pub(super) fn section_entries(mac: bool, entries: &[Entry]) -> std::borrow::Cow<'_, [Entry]> {
    if !mac {
        return entries.into();
    }
    entries
        .iter()
        .copied()
        .filter(|entry| !matches!(entry, Entry::Item(id, ..) if app_entry(id).is_some()))
        .collect::<Vec<_>>()
        .into()
}

pub(super) fn reserved(keys: &str) -> bool {
    let mut parts = keys
        .split('+')
        .map(|part| part.trim().to_ascii_lowercase())
        .collect::<Vec<_>>();
    let key = parts.pop().unwrap_or_default();
    let mut command = false;
    let mut option = false;
    let mut shift = false;
    for part in parts {
        match part.as_str() {
            "cmdorctrl" | "cmdorcontrol" | "commandorctrl" | "commandorcontrol" | "cmd"
            | "command" | "super" | "meta" => command = true,
            "alt" | "option" => option = true,
            "shift" => shift = true,
            _ => return false,
        }
    }
    let key = key
        .strip_prefix("key")
        .filter(|letter| letter.len() == 1)
        .unwrap_or(&key);
    command
        && if option {
            !shift && key == "h"
        } else if shift {
            matches!(key, "tab" | "z" | "grave" | "backtick" | "`" | "backquote")
        } else {
            matches!(
                key,
                "tab"
                    | "a"
                    | "c"
                    | "h"
                    | "m"
                    | "q"
                    | "v"
                    | "w"
                    | "x"
                    | "z"
                    | ","
                    | "comma"
                    | "`"
                    | "backquote"
                    | "grave"
                    | "backtick"
            )
        }
}

pub(super) fn app_menu<R: Runtime>(
    app: &AppHandle<R>,
    overrides: &HashMap<String, String>,
    collected: &mut Collected<R>,
) -> tauri::Result<Submenu<R>> {
    let builder = fill(
        app,
        SubmenuBuilder::with_id(app, APP_ID, "Cogit"),
        APP_ITEMS,
        overrides,
        collected,
    )?
    .separator()
    .services()
    .separator()
    .hide_with_text("Hide Cogit")
    .hide_others()
    .show_all()
    .separator();
    fill(app, builder, &[QUIT], overrides, collected)?.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_sections_move_commands_once_and_protect_native_keys() {
        for id in ["exit", "settings", "about", "check-updates"] {
            assert!(app_entry(id).is_some());
            for (_, entries) in super::super::SECTIONS {
                assert!(
                    !section_entries(true, entries)
                        .iter()
                        .any(|e| matches!(e, Entry::Item(key, ..) if *key == id))
                );
            }
        }
        for key in [
            "CmdOrCtrl+Tab",
            "Shift+Command+Tab",
            "CmdOrCtrl+W",
            "CmdOrCtrl+A",
            "Command+Q",
            "Cmd+,",
            "Option+Cmd+H",
            "Cmd+M",
            "Cmd+Backquote",
            "Shift+Cmd+Grave",
            "CommandOrCtrl+KeyW",
        ] {
            assert!(reserved(key), "{key}");
        }
        for key in [
            "Ctrl+Tab",
            "Ctrl+Shift+Tab",
            "Ctrl+W",
            "CmdOrCtrl+Shift+W",
            "CmdOrCtrl+Alt+M",
        ] {
            assert!(!reserved(key), "{key}");
        }
    }
}
