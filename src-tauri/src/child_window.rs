//! The windows Cogit opens beside the main one: one file being compared, one conflict
//! being resolved.
//!
//! They are not small copies of the main window. They carry no application menu (there is
//! nothing in `Repository ▸ …` that applies to one file), they start dark rather than
//! white so a slow load does not flash a blank page, and they refuse to be made too small
//! to read a diff in (doc/12-risks.md, R-111).

use std::sync::atomic::{AtomicU32, Ordering};

use tauri::utils::config::Color;
use tauri::{WebviewUrl, WebviewWindowBuilder};

/// `--c-bg-window` from the dark theme. A window that has not painted yet shows this
/// instead of the webview's default white.
const BACKGROUND: Color = Color(0x0f, 0x11, 0x15, 0xff);

/// Labels must be unique for the lifetime of the process. Counting the open windows is
/// not enough: close one of two and the next window is handed a label that is still in
/// use by the other, and the build fails.
static NEXT: AtomicU32 = AtomicU32::new(1);

pub struct Shape {
    pub width: f64,
    pub height: f64,
    pub min_width: f64,
    pub min_height: f64,
}

pub fn open<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    kind: &str,
    url: String,
    title: String,
    shape: Shape,
) -> tauri::Result<()> {
    let label = format!("{kind}-{}", NEXT.fetch_add(1, Ordering::Relaxed));
    let empty = tauri::menu::Menu::new(app)?;

    WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
        .title(title)
        .menu(empty)
        .background_color(BACKGROUND)
        .inner_size(shape.width, shape.height)
        .min_inner_size(shape.min_width, shape.min_height)
        .center()
        .build()
        .map(drop)
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
}
