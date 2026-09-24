//! Popup context menus the frontend describes row by row; the menu bar is `super`.

use std::sync::Mutex;

use tauri::menu::{Menu, MenuItemBuilder, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, Runtime};

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
    /// Non-empty makes the row a submenu (`Move To ▸`); its own id is then never chosen.
    #[serde(default)]
    pub children: Vec<ContextItem>,
}

/// `tidy` at every depth; a submenu left empty stays as a disabled row.
#[must_use]
pub fn tidy_items(items: &[ContextItem]) -> Vec<ContextItem> {
    let mut kept: Vec<ContextItem> = Vec::with_capacity(items.len());
    for item in items {
        if item.separator {
            if kept.last().is_none_or(|last| last.separator) {
                continue;
            }
            kept.push(item.clone());
            continue;
        }
        let mut item = item.clone();
        if !item.children.is_empty() {
            item.children = tidy_items(&item.children);
            if item.children.is_empty() {
                item.enabled = false;
            }
        }
        kept.push(item);
    }
    while kept.last().is_some_and(|last| last.separator) {
        kept.pop();
    }
    kept
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
    let menu = Menu::new(app)?;
    for item in tidy_items(items) {
        append_context_item(app, &item, &|entry| menu.append(entry))?;
    }
    window.popup_menu_at(&menu, tauri::LogicalPosition::new(x, y))?;
    if let Ok(mut slot) = held.current.lock() {
        *slot = Some(menu);
    }
    Ok(())
}

type Append<'a, R> = dyn Fn(&dyn tauri::menu::IsMenuItem<R>) -> tauri::Result<()> + 'a;

fn append_context_item<R: Runtime>(
    app: &AppHandle<R>,
    item: &ContextItem,
    append: &Append<'_, R>,
) -> tauri::Result<()> {
    if item.separator {
        return append(&PredefinedMenuItem::separator(app)?);
    }
    if !item.children.is_empty() {
        let nested = Submenu::with_id(app, item.id.as_str(), item.label.as_str(), item.enabled)?;
        for child in &item.children {
            append_context_item(app, child, &|entry| nested.append(entry))?;
        }
        return append(&nested);
    }
    let mut entry =
        MenuItemBuilder::with_id(item.id.as_str(), item.label.as_str()).enabled(item.enabled);
    if let Some(chord) = &item.accelerator {
        entry = entry.accelerator(chord.as_str());
    }
    append(&entry.build(app)?)
}

#[cfg(test)]
mod context_tests {
    use super::*;

    fn parse(json: &str) -> Vec<ContextItem> {
        serde_json::from_str(json).expect("valid context items")
    }

    fn row(id: &str) -> ContextItem {
        ContextItem {
            id: id.to_owned(),
            label: id.to_owned(),
            enabled: true,
            separator: false,
            accelerator: None,
            children: Vec::new(),
        }
    }

    fn line() -> ContextItem {
        ContextItem {
            separator: true,
            ..row("")
        }
    }

    fn shape(items: &[ContextItem]) -> Vec<String> {
        items
            .iter()
            .map(|item| {
                if item.separator {
                    "-".to_owned()
                } else if item.children.is_empty() {
                    item.id.clone()
                } else {
                    format!("{}[{}]", item.id, shape(&item.children).join(","))
                }
            })
            .collect()
    }

    #[test]
    fn a_row_without_children_is_still_a_plain_item() {
        let items = parse(r#"[{"id":"a","label":"A","enabled":true}]"#);
        assert!(items[0].children.is_empty());
        assert!(!items[0].separator);
    }

    #[test]
    fn children_make_a_row_a_submenu() {
        let items = parse(
            r#"[{"id":"move","label":"Move To","enabled":true,
                 "children":[{"id":"move:g1","label":"Work","enabled":true},
                             {"id":"move:g2","label":"Home","enabled":false}]}]"#,
        );
        assert_eq!(shape(&items), ["move[move:g1,move:g2]"]);
        assert!(!items[0].children[1].enabled);
    }

    #[test]
    fn stray_separators_go_at_every_level() {
        let mut nested = row("resolve");
        nested.children = vec![line(), row("theirs"), line(), line(), row("ours"), line()];
        let items = vec![line(), row("a"), line(), line(), nested, line()];
        assert_eq!(
            shape(&tidy_items(&items)),
            ["a", "-", "resolve[theirs,-,ours]"]
        );
    }

    #[test]
    fn a_submenu_left_with_nothing_but_separators_becomes_an_inert_row() {
        let mut empty = row("move");
        empty.children = vec![line(), line()];
        let tidied = tidy_items(&[row("a"), empty]);
        assert_eq!(shape(&tidied), ["a", "move"]);
        assert!(
            !tidied[1].enabled,
            "a submenu with nothing inside offers nothing"
        );
    }
}
