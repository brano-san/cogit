//! The Commit window: the files to commit and the message, in a window of its own.

use crate::child_window::Shape;

/// SmartGit's dialog title.
pub const TITLE: &str = "Commit";

pub const SHAPE: Shape = Shape {
    width: 760.0,
    height: 720.0,
    min_width: 560.0,
    min_height: 560.0,
};

/// In the URL rather than in shared state: the window rebuilds itself after a webview
/// reload (T2.5). `root` keys the message draft the inline Commit Message panel shares.
pub fn url(repo: u32, root: &str) -> String {
    format!("commit.html?repo={repo}&root={}", encode(root))
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
            url(3, r"D:\my repo+x"),
            "commit.html?repo=3&root=D%3A%5Cmy%20repo%2Bx"
        );
    }
}
