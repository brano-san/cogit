// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::legacy_dirs::{Migration, migrate_legacy_dirs};
use std::fs;
use std::path::Path;

fn run(old: &Path, new: &Path) -> Migration {
    migrate_legacy_dirs(&[(old.to_path_buf(), new.to_path_buf())])
        .remove(0)
        .1
}

#[test]
fn an_old_folder_is_renamed_with_its_contents() {
    let tmp = tempfile::tempdir().unwrap();
    let (old, new) = (
        tmp.path().join("dev.branosan.cogit"),
        tmp.path().join("Cogit"),
    );
    fs::create_dir_all(old.join("EBWebView")).unwrap();
    fs::write(old.join("settings.json"), "{}").unwrap();

    assert_eq!(run(&old, &new), Migration::Renamed);
    assert!(!old.exists());
    assert!(new.join("EBWebView").is_dir() && new.join("settings.json").is_file());
}

#[test]
fn a_missing_old_folder_is_nothing_to_do_and_a_second_run_is_too() {
    let tmp = tempfile::tempdir().unwrap();
    let (old, new) = (tmp.path().join("old"), tmp.path().join("Cogit"));
    assert_eq!(run(&old, &new), Migration::Absent);

    fs::create_dir_all(&old).unwrap();
    assert_eq!(run(&old, &new), Migration::Renamed);
    assert_eq!(run(&old, &new), Migration::Absent);
}

#[test]
fn an_existing_new_folder_takes_only_the_entries_it_lacks() {
    let tmp = tempfile::tempdir().unwrap();
    let (old, new) = (tmp.path().join("old"), tmp.path().join("Cogit"));
    fs::create_dir_all(old.join("logs")).unwrap();
    fs::write(old.join("clash"), "old").unwrap();
    fs::create_dir_all(&new).unwrap();
    fs::write(new.join("cogit.exe"), "").unwrap();
    fs::write(new.join("clash"), "new").unwrap();

    let Migration::Merged { moved, left } = run(&old, &new) else {
        panic!("expected a merge");
    };
    assert_eq!(
        (moved, left),
        (vec!["logs".to_owned()], vec!["clash".to_owned()])
    );
    assert_eq!(fs::read_to_string(new.join("clash")).unwrap(), "new");
    assert_eq!(fs::read_to_string(old.join("clash")).unwrap(), "old");
    assert!(new.join("logs").is_dir() && new.join("cogit.exe").is_file());
}

#[test]
fn an_old_folder_emptied_by_the_merge_is_removed() {
    let tmp = tempfile::tempdir().unwrap();
    let (old, new) = (tmp.path().join("old"), tmp.path().join("Cogit"));
    fs::create_dir_all(old.join("avatars")).unwrap();
    fs::create_dir_all(&new).unwrap();

    assert!(matches!(run(&old, &new), Migration::Merged { .. }));
    assert!(!old.exists());
}

#[test]
fn a_failed_rename_leaves_the_old_folder_untouched() {
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("old");
    fs::create_dir_all(&old).unwrap();
    fs::write(old.join("settings.json"), "{}").unwrap();
    let new = tmp.path().join("no-such-parent").join("Cogit");

    assert!(matches!(run(&old, &new), Migration::Failed(_)));
    assert!(old.join("settings.json").is_file());
}
