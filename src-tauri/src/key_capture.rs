//! Preferences ▸ Keyboard records a shortcut from the page, but the window takes any key
//! its menu has before the page sees it (`webview2::install_accelerators`) and runs the
//! command instead. While a shortcut is being recorded, it takes none.

use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub struct KeyCapture(AtomicBool);

impl KeyCapture {
    pub fn set(&self, on: bool) {
        self.0.store(on, Ordering::SeqCst);
    }

    /// What the menu claims for a key, or nothing while one is being recorded.
    pub fn claim<T>(&self, claim: impl FnOnce() -> Option<T>) -> Option<T> {
        if self.0.load(Ordering::SeqCst) {
            None
        } else {
            claim()
        }
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

    #[test]
    fn the_menu_has_its_keys_back_once_recording_ends() {
        let capture = KeyCapture::default();
        capture.set(true);
        capture.set(false);

        assert_eq!(capture.claim(|| Some("stash")), Some("stash"));
    }
}
