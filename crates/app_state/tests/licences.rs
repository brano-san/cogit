// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::licences::{document, write};

const CRATES: &str = "Rust crates (1)\n\ngix 0.87.1 — MIT OR Apache-2.0\n";
const FRONTEND: &str = "Frontend packages (1)\n\nsvelte 5.57.1 — MIT\n";

#[test]
fn the_document_names_the_build_and_carries_both_lists() {
    let text = document("0.1.0", CRATES, Some(FRONTEND));
    assert_eq!(
        text.lines().next().unwrap(),
        "Third-party licenses in Cogit 0.1.0"
    );
    assert!(text.contains("gix 0.87.1 — MIT OR Apache-2.0"));
    assert!(text.contains("svelte 5.57.1 — MIT"));
}

#[test]
fn a_build_without_a_frontend_list_says_where_to_find_one() {
    let text = document("0.1.0", CRATES, None);
    assert!(text.contains("gix 0.87.1"));
    assert!(text.contains("Frontend packages"));
    assert!(text.contains("release build"));
}

#[test]
fn the_document_is_written_where_asked_and_can_be_written_again() {
    let dir = tempfile::tempdir().unwrap();
    let first = write(dir.path(), "one").unwrap();
    let second = write(dir.path(), "two").unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first.file_name().unwrap(),
        "cogit-third-party-licenses.txt",
        "the editor shows the name"
    );
    assert_eq!(std::fs::read_to_string(second).unwrap(), "two");
}
