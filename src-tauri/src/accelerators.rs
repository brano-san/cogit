//! Which key combinations belong to the window, and which belong to the page.
//!
//! On Windows a menu accelerator is matched by the window's own message loop. WebView2
//! never gives it the chance: while the page has focus every key goes into the webview,
//! so `Ctrl+1` reaches the document and the menu item never fires (problem 3).
//!
//! The cure is to look at accelerator keys before the page does — but only at the ones the
//! menu actually claims. Everything else has to pass through untouched, because the panels
//! rely on `Ctrl+A`, the arrows, `Enter` and `Ctrl+F` reaching them.
//!
//! This module is the decision, with no Windows API in it, so the rule can be tested.

use std::collections::HashMap;

/// A pressed combination, reduced to what a menu accelerator can express.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// A Windows virtual-key code. Plain numbers here rather than the `windows` crate:
    /// the values are stable and this way the rule is testable off Windows.
    pub key: u16,
}

/// The modifier keys as the keyboard reports them, with the right Alt apart: on a layout
/// that has AltGr it arrives as Ctrl plus right Alt, and it types a character.
#[derive(Debug, Clone, Copy, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub right_alt: bool,
}

/// The chord a key press stands for, or `None` for a press that is typing: AltGr+S is `ś`
/// on a Polish keyboard, not `CmdOrCtrl+Alt+S`.
#[must_use]
pub fn chord_of(modifiers: Modifiers, key: u16) -> Option<Chord> {
    if modifiers.right_alt && modifiers.ctrl {
        return None;
    }
    Some(Chord {
        ctrl: modifiers.ctrl,
        shift: modifiers.shift,
        alt: modifiers.alt,
        key,
    })
}

const VK_OEM_COMMA: u16 = 0xBC;
const VK_OEM_PERIOD: u16 = 0xBE;
const VK_OEM_MINUS: u16 = 0xBD;
const VK_OEM_PLUS: u16 = 0xBB;
const VK_F1: u16 = 0x70;
const VK_RETURN: u16 = 0x0D;
const VK_LEFT: u16 = 0x25;
const VK_UP: u16 = 0x26;
const VK_RIGHT: u16 = 0x27;
const VK_DOWN: u16 = 0x28;

/// One accelerator as the menu writes it: `CmdOrCtrl+Shift+7`, `Shift+F11`, `F5`.
///
/// `None` for anything this cannot express — a combination that cannot be recognised must
/// stay with the page rather than be claimed and swallowed.
#[must_use]
pub fn parse(accelerator: &str) -> Option<Chord> {
    let mut chord = Chord {
        ctrl: false,
        shift: false,
        alt: false,
        key: 0,
    };

    let mut named = None;
    for part in accelerator.split('+') {
        let part = part.trim();
        // An empty part means a stray or trailing `+`. No menu key is spelled that way,
        // and a combination that cannot be read must stay with the page.
        if part.is_empty() {
            return None;
        }

        match part.to_ascii_lowercase().as_str() {
            "cmdorctrl" | "commandorcontrol" | "ctrl" | "control" | "cmd" | "command" => {
                chord.ctrl = true;
            }
            "shift" => chord.shift = true,
            "alt" | "option" => chord.alt = true,
            _ => {
                if named.is_some() {
                    return None; // Two keys in one accelerator: not something to claim.
                }
                named = Some(part);
            }
        }
    }

    chord.key = virtual_key(named?)?;
    // Without Ctrl or Alt a key is typing or moving about the page; only an F-key is a
    // command on its own.
    if !chord.ctrl && !chord.alt && !(VK_F1..VK_F1 + 24).contains(&chord.key) {
        return None;
    }
    Some(chord)
}

fn virtual_key(name: &str) -> Option<u16> {
    let upper = name.to_ascii_uppercase();

    if let Some(number) = upper.strip_prefix('F')
        && let Ok(index) = number.parse::<u16>()
        && (1..=24).contains(&index)
    {
        return Some(VK_F1 + index - 1);
    }

    match upper.as_str() {
        // muda reads only `Enter`; `Return` is what an older keymap editor recorded.
        "ENTER" | "RETURN" => return Some(VK_RETURN),
        "LEFT" => return Some(VK_LEFT),
        "UP" => return Some(VK_UP),
        "RIGHT" => return Some(VK_RIGHT),
        "DOWN" => return Some(VK_DOWN),
        _ => {}
    }

    let mut chars = upper.chars();
    let (first, rest) = (chars.next()?, chars.next());
    if rest.is_some() {
        return None;
    }

    match first {
        '0'..='9' => Some(first as u16),
        'A'..='Z' => Some(first as u16),
        ',' => Some(VK_OEM_COMMA),
        '.' => Some(VK_OEM_PERIOD),
        '-' => Some(VK_OEM_MINUS),
        '+' | '=' => Some(VK_OEM_PLUS),
        _ => None,
    }
}

/// Every combination the window claims, mapped to the menu id it stands for.
///
/// `bindings` is `(id, default accelerator)`; `overrides` is what the user chose, where an
/// empty string means "no key at all" — the same rule the menu builder follows.
#[must_use]
pub fn table<'a>(
    bindings: impl IntoIterator<Item = (&'a str, Option<&'a str>)>,
    overrides: &HashMap<String, String>,
) -> HashMap<Chord, String> {
    let mut claimed = HashMap::new();

    for (id, default) in bindings {
        let accelerator = match overrides.get(id) {
            Some(keys) if keys.is_empty() => continue,
            Some(keys) => keys.as_str(),
            None => match default {
                Some(keys) => keys,
                None => continue,
            },
        };

        if let Some(chord) = parse(accelerator) {
            claimed.insert(chord, id.to_owned());
        }
    }

    claimed
}

#[cfg(test)]
mod tests {
    use super::*;

    // AltGr arrives as left Ctrl with right Alt. Read as Ctrl+Alt it matched Stash
    // Selection, and the `ś` being typed into the commit message was swallowed.
    #[test]
    fn a_character_typed_with_altgr_is_not_a_shortcut() {
        let altgr = Modifiers {
            ctrl: true,
            alt: true,
            right_alt: true,
            ..Modifiers::default()
        };
        assert_eq!(chord_of(altgr, u16::from(b'S')), None);

        let ctrl_alt = Modifiers {
            ctrl: true,
            alt: true,
            ..Modifiers::default()
        };
        assert_eq!(
            chord_of(ctrl_alt, u16::from(b'S')),
            parse("CmdOrCtrl+Alt+S")
        );
    }

    fn no_overrides() -> HashMap<String, String> {
        HashMap::new()
    }

    #[test]
    fn a_digit_accelerator_is_recognised() {
        let chord = parse("CmdOrCtrl+1").expect("parses");
        assert!(chord.ctrl && !chord.shift && !chord.alt);
        assert_eq!(chord.key, u16::from(b'1'));
    }

    #[test]
    fn modifiers_stack() {
        let chord = parse("CmdOrCtrl+Shift+7").expect("parses");
        assert!(chord.ctrl && chord.shift && !chord.alt);
        assert_eq!(chord.key, u16::from(b'7'));

        let chord = parse("CmdOrCtrl+Alt+S").expect("parses");
        assert!(chord.ctrl && chord.alt && !chord.shift);
    }

    #[test]
    fn function_keys_map_to_their_codes() {
        assert_eq!(parse("F5").expect("parses").key, 0x74);
        let chord = parse("Shift+F11").expect("parses");
        assert!(chord.shift && !chord.ctrl);
        assert_eq!(chord.key, 0x7A);
    }

    // Investigate's Back, Forward, Newer and Older Version are Alt and an arrow. Read as
    // nothing, they were never claimed, and WebView2 kept them from the menu.
    #[test]
    fn arrow_keys_are_recognised() {
        let chord = parse("Alt+Left").expect("parses");
        assert!(chord.alt && !chord.ctrl && !chord.shift);
        assert_eq!(chord.key, 0x25);
        assert_eq!(parse("Alt+Up").expect("parses").key, 0x26);
        assert_eq!(parse("Alt+Right").expect("parses").key, 0x27);
        assert_eq!(parse("Alt+Down").expect("parses").key, 0x28);
    }

    // Local ▸ Commit… was `CmdOrCtrl+Return`, which neither muda nor this module reads:
    // the item showed no key, and Ctrl+Enter reached the commit field alone.
    #[test]
    fn enter_is_recognised() {
        let chord = parse("CmdOrCtrl+Enter").expect("parses");
        assert!(chord.ctrl && !chord.shift && !chord.alt);
        assert_eq!(chord.key, 0x0D);
        assert_eq!(parse("CmdOrCtrl+Shift+Enter").expect("parses").key, 0x0D);
    }

    /// A default the table cannot read is a key the window never claims.
    #[test]
    fn every_default_accelerator_can_be_claimed() {
        for (id, keys) in crate::menu::default_keymap_pairs() {
            if let Some(keys) = keys {
                assert!(parse(keys).is_some(), "{id}: {keys}");
            }
        }
    }

    #[test]
    fn punctuation_the_menu_uses_is_recognised() {
        assert_eq!(parse("CmdOrCtrl+,").expect("parses").key, VK_OEM_COMMA);
    }

    #[test]
    fn something_unrecognised_is_left_to_the_page() {
        assert!(parse("CmdOrCtrl+Space").is_none());
        assert!(parse("CmdOrCtrl").is_none());
        assert!(parse("").is_none());
    }

    // A recorded `Up` or a lone letter would be claimed from every panel and text field.
    #[test]
    fn a_key_without_ctrl_or_alt_is_left_to_the_page_unless_it_is_an_f_key() {
        assert!(parse("Up").is_none());
        assert!(parse("Shift+A").is_none());
        assert!(parse("Enter").is_none());
        assert!(parse("F24").is_some());
        assert!(parse("Shift+F6").is_some());
    }

    // The keymap editor showed Ctrl+/ or Ctrl+Space as assigned, and the key did nothing.
    // It records only what this reads: one table of cases for both sides, `claimable` in
    // `lib/keymap.ts` on the other.
    #[test]
    fn the_keymap_editor_and_the_window_agree_on_what_can_be_claimed() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../frontend/src/lib/accelerator-cases.json"
        ))
        .expect("the shared cases are JSON");
        for (list, claimed) in [("claimable", true), ("refused", false)] {
            let keys = cases[list].as_array().expect("a list of accelerators");
            assert!(!keys.is_empty());
            for keys in keys {
                let keys = keys.as_str().expect("an accelerator");
                assert_eq!(parse(keys).is_some(), claimed, "{keys:?}");
            }
        }
    }

    #[test]
    fn the_table_claims_only_what_the_menu_declares() {
        let claimed = table(
            [("panel-graph", Some("CmdOrCtrl+3")), ("blame", None)],
            &no_overrides(),
        );
        assert_eq!(claimed.len(), 1);
        assert_eq!(
            claimed.get(&parse("CmdOrCtrl+3").expect("parses")),
            Some(&"panel-graph".to_owned())
        );
    }

    #[test]
    fn an_override_replaces_the_default() {
        let overrides = HashMap::from([("panel-graph".to_owned(), "CmdOrCtrl+9".to_owned())]);
        let claimed = table([("panel-graph", Some("CmdOrCtrl+3"))], &overrides);

        assert!(!claimed.contains_key(&parse("CmdOrCtrl+3").expect("parses")));
        assert_eq!(
            claimed.get(&parse("CmdOrCtrl+9").expect("parses")),
            Some(&"panel-graph".to_owned())
        );
    }

    #[test]
    fn an_empty_override_gives_the_key_back_to_the_page() {
        let overrides = HashMap::from([("panel-graph".to_owned(), String::new())]);
        assert!(table([("panel-graph", Some("CmdOrCtrl+3"))], &overrides).is_empty());
    }

    /// The division of labour with the panels: these belong to whatever has focus, and the
    /// window must never take them. None of them is a menu accelerator, so the table is
    /// what keeps that true — this test fails the moment one is added.
    #[test]
    fn the_keys_the_panels_need_are_never_claimed() {
        let claimed = table(crate::menu::default_keymap_pairs(), &no_overrides());

        for keys in [
            "CmdOrCtrl+A",
            "CmdOrCtrl+F",
            "CmdOrCtrl+C",
            "CmdOrCtrl+V",
            "CmdOrCtrl+X",
            "CmdOrCtrl+Z",
            "CmdOrCtrl+Shift+Z",
            "CmdOrCtrl+Y",
        ] {
            let chord = parse(keys).expect("parses");
            assert!(!claimed.contains_key(&chord), "{keys} must reach the panel");
        }
    }
}
