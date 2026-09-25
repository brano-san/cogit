// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::settings::{read_document, write_key};
use serde_json::json;

fn dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn a_missing_file_reads_as_an_empty_document() {
    let home = dir();
    assert_eq!(read_document(home.path()), json!({}));
}

#[test]
fn a_written_key_reads_back() {
    let home = dir();
    write_key(home.path(), "settings", json!({ "theme": "dark" })).unwrap();

    assert_eq!(
        read_document(home.path())["settings"]["theme"],
        json!("dark")
    );
}

#[test]
fn writing_one_key_leaves_the_others_alone() {
    let home = dir();
    write_key(home.path(), "settings", json!({ "theme": "dark" })).unwrap();
    write_key(home.path(), "keymap", json!({ "commit": "Ctrl+Enter" })).unwrap();

    let document = read_document(home.path());
    assert_eq!(document["settings"]["theme"], json!("dark"));
    assert_eq!(document["keymap"]["commit"], json!("Ctrl+Enter"));
}

#[test]
fn a_second_write_of_the_same_key_replaces_it() {
    let home = dir();
    write_key(home.path(), "settings", json!({ "theme": "dark" })).unwrap();
    write_key(home.path(), "settings", json!({ "theme": "light" })).unwrap();

    assert_eq!(
        read_document(home.path())["settings"]["theme"],
        json!("light")
    );
}

#[test]
fn a_damaged_file_reads_as_empty_rather_than_failing() {
    let home = dir();
    std::fs::write(home.path().join("settings.json"), "{ this is not json").unwrap();

    assert_eq!(read_document(home.path()), json!({}));
}

#[test]
fn a_damaged_file_is_replaced_by_the_next_write_rather_than_blocking_it() {
    let home = dir();
    std::fs::write(home.path().join("settings.json"), "{ this is not json").unwrap();

    write_key(home.path(), "settings", json!({ "theme": "dark" })).unwrap();
    assert_eq!(
        read_document(home.path())["settings"]["theme"],
        json!("dark")
    );
}

#[test]
fn the_write_leaves_no_temporary_file_behind() {
    let home = dir();
    write_key(home.path(), "settings", json!({ "theme": "dark" })).unwrap();

    let left: Vec<_> = std::fs::read_dir(home.path())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(left, vec!["settings.json".to_owned()], "{left:?}");
}

#[test]
fn a_missing_directory_is_created_on_the_way() {
    let home = dir();
    let nested = home.path().join("config").join("cogit");

    write_key(&nested, "settings", json!({ "theme": "dark" })).unwrap();
    assert_eq!(read_document(&nested)["settings"]["theme"], json!("dark"));
}

/// The About window names this file; it has to be the one the writes land in.
#[test]
fn the_settings_path_is_the_file_that_was_written() {
    let home = dir();
    write_key(home.path(), "settings", json!({ "theme": "dark" })).unwrap();

    let path = app_state::settings::path(home.path());
    assert!(path.is_file(), "{} was not written", path.display());
    assert_eq!(path.parent(), Some(home.path()));
}

/// The three windows write the same file. A read-modify-write in one process is enough
/// to keep them from erasing each other's keys.
#[test]
fn concurrent_writers_do_not_erase_each_other() {
    let home = dir();
    let root = home.path().to_path_buf();

    std::thread::scope(|scope| {
        for index in 0..8 {
            let root = root.clone();
            scope.spawn(move || {
                write_key(&root, &format!("key{index}"), json!(index)).unwrap();
            });
        }
    });

    let document = read_document(&root);
    for index in 0..8 {
        assert_eq!(document[format!("key{index}")], json!(index), "{document}");
    }
}

// One stray comma in a hand-edited file and the next change of any setting wrote a
// document holding that one key: every other setting was gone, with no copy left.
#[test]
fn a_damaged_file_is_kept_aside_before_the_next_write_replaces_it() {
    let home = dir();
    let damaged = "{ \"settings\": { \"theme\": \"dark\", }, \"keymap\": {} }";
    std::fs::write(home.path().join("settings.json"), damaged).unwrap();

    write_key(home.path(), "window", json!({ "maximised": true })).unwrap();

    assert_eq!(
        std::fs::read_to_string(home.path().join("settings.json.damaged")).unwrap(),
        damaged
    );
}

// Saved by a text editor that writes a byte-order mark, settings.json no longer parsed: it
// was put aside as damaged and every setting went back to its default.
#[test]
fn a_byte_order_mark_does_not_make_the_settings_damaged() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("settings.json"), "\u{feff}{\"a\":1}").unwrap();

    app_state::settings::write_key(dir.path(), "b", serde_json::json!(2)).unwrap();

    let document = app_state::settings::read_document(dir.path());
    assert_eq!(document["a"], 1, "{document}");
    assert_eq!(document["b"], 2, "{document}");
}
#[test]
fn the_git_executable_is_read_from_the_settings_file() {
    let dir = tempfile::tempdir().unwrap();
    app_state::settings::write_key(
        dir.path(),
        "settings",
        serde_json::json!({ "gitPath": "D:/PortableGit/cmd/git.exe" }),
    )
    .unwrap();

    assert_eq!(
        app_state::settings::read_git_program(dir.path()),
        Some(std::path::PathBuf::from("D:/PortableGit/cmd/git.exe"))
    );
}

#[test]
fn plain_git_or_nothing_means_the_one_on_path() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(app_state::settings::read_git_program(dir.path()), None);

    app_state::settings::write_key(
        dir.path(),
        "settings",
        serde_json::json!({ "gitPath": " git " }),
    )
    .unwrap();
    assert_eq!(app_state::settings::read_git_program(dir.path()), None);
}
