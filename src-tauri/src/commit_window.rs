//! The Commit window: the files to commit and the message, in a window of its own.

use crate::child_window::Shape;

/// The dialog title.
pub const TITLE: &str = "Commit";

pub const SHAPE: Shape = Shape {
    width: 760.0,
    height: 720.0,
    min_width: 560.0,
    min_height: 560.0,
};

/// In the URL rather than in shared state: the window rebuilds itself after a webview
/// reload (T2.5). `root` keys the message draft the inline Commit Message panel shares;
/// `paths`, the files it was opened for (Files ▸ Commit…), none for all of them.
pub fn url(repo: u32, root: &str, paths: &[String]) -> String {
    let mut url = format!("commit.html?repo={repo}&root={}", encode(root));
    for path in paths {
        url.push_str("&path=");
        url.push_str(&encode(path));
    }
    url
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_url_carries_the_repository_and_survives_a_windows_root() {
        assert_eq!(
            url(3, r"D:\my repo+x", &[]),
            "commit.html?repo=3&root=D%3A%5Cmy%20repo%2Bx"
        );
    }

    #[test]
    fn the_url_carries_each_file_the_window_is_for() {
        let paths = ["a b.txt".to_owned(), "dir/c+d&e".to_owned()];
        assert_eq!(
            url(3, "r", &paths),
            "commit.html?repo=3&root=r&path=a%20b.txt&path=dir/c%2Bd%26e"
        );
    }
}
