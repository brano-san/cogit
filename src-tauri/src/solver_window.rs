//! The Conflict Solver window: one conflicted file, three panes, in a window of its own.

pub const TITLE_PREFIX: &str = "Cogit — Conflict Solver — ";

#[must_use]
pub fn title(path: &str) -> String {
    format!("{TITLE_PREFIX}{path}")
}

/// In the URL rather than in shared state: the window rebuilds itself after a webview
/// reload (T2.5), and it is also how an open window for the same file is recognized.
#[must_use]
pub fn url(repo: u32, path: &str, external_tool: bool) -> String {
    let tool = if external_tool { "&tool=1" } else { "" };
    format!("solver.html?repo={repo}&path={}{tool}", encode(path))
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

/// Whether an open window shows this file, read from its address as the webview reports it.
#[must_use]
pub fn is_window_for(open: &tauri::Url, repo: u32, path: &str) -> bool {
    if !open.path().ends_with("/solver.html") {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_names_the_window_and_the_file() {
        assert_eq!(
            title("src/device/plate_navboard2.cpp"),
            "Cogit — Conflict Solver — src/device/plate_navboard2.cpp"
        );
    }

    #[test]
    fn the_url_asks_for_the_external_tool_only_when_told_to() {
        assert!(url(1, "a", true).ends_with("&tool=1"));
        assert!(!url(1, "a", false).contains("tool"));
    }

    #[test]
    fn the_url_survives_any_character_a_path_can_hold() {
        assert_eq!(
            url(3, "папка/a b&c#d+e.rs", false),
            "solver.html?repo=3&path=%D0%BF%D0%B0%D0%BF%D0%BA%D0%B0/a%20b%26c%23d%2Be.rs"
        );
    }

    #[test]
    fn an_open_window_is_recognized_by_its_repository_and_file_whatever_its_origin() {
        let open = |address: &str| tauri::Url::parse(address).unwrap();
        let here = "src/a b.rs";
        for origin in ["http://localhost:1420", "http://tauri.localhost"] {
            let address = format!("{origin}/{}", url(1, here, true));
            assert!(is_window_for(&open(&address), 1, here));
        }
        let page = "http://tauri.localhost/solver.html";
        assert!(!is_window_for(
            &open(&format!("{page}?repo=1&path=src/other.rs")),
            1,
            here
        ));
        assert!(!is_window_for(
            &open(&format!("{page}?repo=2&path=src/a%20b.rs")),
            1,
            here
        ));
        assert!(!is_window_for(
            &open("http://tauri.localhost/blame.html?repo=1&path=src/a%20b.rs"),
            1,
            here
        ));
    }
}
