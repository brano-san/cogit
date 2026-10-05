//! The Edit window: one working-tree file in the built-in editor, in a window of its own.
//! One window per file: Edit on a file already open brings its window forward.

use git_engine::GitError;
use parking_lot::Mutex;
use tauri::Manager as _;

use crate::child_window::{self, Shape};

pub const SHAPE: Shape = Shape {
    width: 1100.0,
    height: 800.0,
    min_width: 600.0,
    min_height: 400.0,
};

/// `<file> — Edit — Cogit`; the page adds ` *` while there are unsaved edits.
#[must_use]
pub fn title(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    format!("{name} — Edit — Cogit")
}

/// In the URL rather than in shared state: the window rebuilds itself after a webview
/// reload (T2.5), and it is also how an open window for the same file is recognized.
#[must_use]
pub fn url(repo: u32, path: &str) -> String {
    format!("editor.html?repo={repo}&path={}", encode(path))
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

/// Whether an open window edits this file, read from its address as the webview reports it.
#[must_use]
pub fn is_window_for(open: &tauri::Url, repo: u32, path: &str) -> bool {
    if !open.path().ends_with("/editor.html") {
        return false;
    }
    let mut same_repo = false;
    let mut same_path = false;
    for (key, value) in open.query_pairs() {
        match key.as_ref() {
            "repo" => same_repo = value == repo.to_string(),
            "path" => same_path = value == path,
            _ => {}
        }
    }
    same_repo && same_path
}

/// Which window edits which file, by label. Its address alone is not enough: a window just
/// built has none yet, and a second Edit right after the first opened a second window.
static OPEN: Mutex<Vec<(u32, String, String)>> = Mutex::new(Vec::new());

/// Brings forward the window already editing this file, or opens one on the main
/// window's monitor. Blocks while the window is built, so never call it on the main thread.
pub fn reveal_or_open(app: &tauri::AppHandle, repo: u32, path: &str) -> Result<(), GitError> {
    let mut open = OPEN.lock();
    open.retain(|(_, _, label)| app.get_webview_window(label).is_some());
    let known = open
        .iter()
        .find(|(r, p, _)| *r == repo && p == path)
        .and_then(|(_, _, label)| app.get_webview_window(label));
    let found = known.or_else(|| {
        app.webview_windows()
            .into_iter()
            .find_map(|(label, window)| {
                let shown = window
                    .url()
                    .is_ok_and(|url| is_window_for(&url, repo, path));
                (label.starts_with("editor-") && shown).then_some(window)
            })
    });
    if let Some(window) = found {
        return window
            .unminimize()
            .and_then(|()| window.set_focus())
            .map_err(|err| GitError::Internal(format!("cannot focus the editor: {err}")));
    }
    child_window::open(app, "editor", url(repo, path), title(path), SHAPE)
        .map_err(|err| GitError::Internal(format!("cannot open the editor: {err}")))?;
    // The newest editor window is the one just built: labels count up (`editor-N`).
    let newest = app
        .webview_windows()
        .into_keys()
        .filter(|label| !open.iter().any(|(_, _, known)| known == label))
        .filter_map(|label| Some((label.strip_prefix("editor-")?.parse::<u64>().ok()?, label)))
        .max();
    if let Some((_, label)) = newest {
        open.push((repo, path.to_owned(), label));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_names_the_file() {
        assert_eq!(title("src/lib/a b.rs"), "a b.rs — Edit — Cogit");
    }

    #[test]
    fn an_open_window_is_recognized_by_its_repository_and_file() {
        let open = |address: &str| tauri::Url::parse(address).unwrap();
        let here = "src/a b.rs";
        let address = format!("http://tauri.localhost/{}", url(1, here));
        assert!(is_window_for(&open(&address), 1, here));
        assert!(!is_window_for(&open(&address), 2, here));
        assert!(!is_window_for(
            &open("http://tauri.localhost/solver.html?repo=1&path=src/a%20b.rs"),
            1,
            here
        ));
    }
}
