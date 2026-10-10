//! Preferences ▸ Keyboard records a shortcut from the page, but the window takes any key
//! its menu has before the page sees it (`webview2::install_accelerators`) and runs the
//! command instead. While a shortcut is being recorded, it takes none; under a modal
//! dialog, only Exit's, the one command that still runs there (R-451).

// Read only by the Windows accelerator hook.
#![cfg_attr(not(windows), allow(dead_code))]

use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub struct KeyCapture {
    recording: AtomicBool,
    modal: AtomicBool,
}

impl KeyCapture {
    pub fn set(&self, on: bool) {
        self.recording.store(on, Ordering::SeqCst);
    }

    pub fn set_modal(&self, open: bool) {
        self.modal.store(open, Ordering::SeqCst);
    }

    /// What the menu claims for a key: nothing while one is being recorded, and under a
    /// modal nothing but Exit, so `Ctrl+Enter` reaches the dialog.
    pub fn claim<T: AsRef<str>>(&self, claim: impl FnOnce() -> Option<T>) -> Option<T> {
        if self.recording.load(Ordering::SeqCst) {
            return None;
        }
        claim().filter(|id| !self.modal.load(Ordering::SeqCst) || id.as_ref() == "exit")
    }
}

#[cfg(test)]
mod tests {
    use super::KeyCapture;

    #[test]
    fn a_key_the_menu_has_reaches_the_page_while_one_is_recorded() {
        let capture = KeyCapture::default();
        capture.set(true);

        assert_eq!(capture.claim(|| Some("stash")), None);
    }

    // Ctrl+Enter in a dialog ran Commit, which a modal drops, and the dialog never saw it.
    #[test]
    fn under_a_modal_the_menu_keeps_only_exit() {
        let table = crate::accelerators::table(
            crate::menu::default_keymap_pairs(),
            &std::collections::HashMap::new(),
        );
        let claimed = |capture: &KeyCapture, keys: &str| {
            let chord = crate::accelerators::parse(keys).expect("parses");
            capture.claim(|| table.get(&chord).cloned())
        };
        let capture = KeyCapture::default();
        capture.set_modal(true);

        assert_eq!(claimed(&capture, "CmdOrCtrl+Enter"), None);
        assert_eq!(claimed(&capture, "Alt+X").as_deref(), Some("exit"));

        capture.set_modal(false);
        assert_eq!(
            claimed(&capture, "CmdOrCtrl+Enter").as_deref(),
            Some("commit")
        );
    }

    #[test]
    fn the_menu_has_its_keys_back_once_recording_ends() {
        let capture = KeyCapture::default();
        capture.set(true);
        capture.set(false);

        assert_eq!(capture.claim(|| Some("stash")), Some("stash"));
    }
}
