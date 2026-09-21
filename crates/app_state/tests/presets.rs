#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The preset catalogue as the Hooks panel sees it: built-ins plus the user's own, each
//! told against one repository so a missing config file is visible before install (M10).

use app_state::{AppState, RepoId};

fn opened(f: &test_fixtures::Fixture) -> (AppState, RepoId) {
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    (state, repo)
}

#[test]
fn a_preset_whose_config_file_is_absent_says_so() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = opened(&f);

    let presets = state.presets_for(repo).unwrap();
    let with_config = presets
        .iter()
        .find(|preset| !preset.config_files.is_empty())
        .expect("a built-in preset declares a config file");

    assert_eq!(with_config.missing_config, with_config.config_files);
}

#[test]
fn a_config_file_that_is_there_is_not_reported_missing() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = opened(&f);

    let wanted = state
        .presets_for(repo)
        .unwrap()
        .into_iter()
        .find(|preset| !preset.config_files.is_empty())
        .expect("a built-in preset declares a config file");
    std::fs::write(f.path().join(&wanted.config_files[0]), "").unwrap();

    let after = state.presets_for(repo).unwrap();
    let same = after.iter().find(|p| p.id == wanted.id).unwrap();
    assert!(!same.missing_config.contains(&wanted.config_files[0]));
}

#[test]
fn a_preset_without_config_files_is_never_short_of_one() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = opened(&f);

    for preset in state.presets_for(repo).unwrap() {
        if preset.config_files.is_empty() {
            assert!(preset.missing_config.is_empty());
        }
    }
}

#[test]
fn the_built_in_catalogue_is_marked_as_built_in() {
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = opened(&f);
    assert!(state.presets_for(repo).unwrap().iter().all(|p| !p.user));
}

#[test]
fn a_hook_can_be_saved_as_a_preset_of_the_users_own() {
    let dir = tempfile::tempdir().unwrap();
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = opened(&f);
    state.use_preset_dir(dir.path().to_path_buf());

    state
        .write_hook(repo, "pre-commit", "#!/bin/sh\necho mine\n")
        .unwrap();
    state
        .export_preset(repo, "pre-commit", "mine", "My check", "Runs my own check")
        .unwrap();

    let saved = state
        .presets_for(repo)
        .unwrap()
        .into_iter()
        .find(|preset| preset.id == "mine")
        .expect("the exported preset is in the catalogue");
    assert!(saved.user);
    assert_eq!(saved.name, "My check");
    assert_eq!(saved.hook, "pre-commit");
}

#[test]
fn an_exported_preset_installs_the_script_it_was_made_from() {
    let dir = tempfile::tempdir().unwrap();
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = opened(&f);
    state.use_preset_dir(dir.path().to_path_buf());

    state
        .write_hook(repo, "pre-commit", "#!/bin/sh\necho mine\n")
        .unwrap();
    state
        .export_preset(repo, "pre-commit", "mine", "My check", "")
        .unwrap();
    state.write_hook(repo, "pre-commit", "#!/bin/sh\n").unwrap();

    state.install_preset(repo, "mine").unwrap();

    assert!(
        state
            .read_hook(repo, "pre-commit")
            .unwrap()
            .contains("echo mine")
    );
}

#[test]
fn exporting_a_hook_that_is_not_there_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = opened(&f);
    state.use_preset_dir(dir.path().to_path_buf());

    assert!(
        state
            .export_preset(repo, "pre-push", "nope", "Nope", "")
            .is_err()
    );
}

#[test]
fn an_exported_preset_cannot_take_the_name_of_a_built_in_one() {
    let dir = tempfile::tempdir().unwrap();
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = opened(&f);
    state.use_preset_dir(dir.path().to_path_buf());
    state.write_hook(repo, "pre-commit", "#!/bin/sh\n").unwrap();

    let taken = state.presets_for(repo).unwrap()[0].id.clone();
    assert!(
        state
            .export_preset(repo, "pre-commit", &taken, "Clash", "")
            .is_err()
    );
}

#[test]
fn a_users_preset_can_be_removed_again() {
    let dir = tempfile::tempdir().unwrap();
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = opened(&f);
    state.use_preset_dir(dir.path().to_path_buf());
    state.write_hook(repo, "pre-commit", "#!/bin/sh\n").unwrap();
    state
        .export_preset(repo, "pre-commit", "mine", "My check", "")
        .unwrap();

    state.remove_preset("mine").unwrap();

    assert!(
        state
            .presets_for(repo)
            .unwrap()
            .iter()
            .all(|p| p.id != "mine")
    );
}

#[test]
fn a_built_in_preset_cannot_be_removed() {
    let dir = tempfile::tempdir().unwrap();
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = opened(&f);
    state.use_preset_dir(dir.path().to_path_buf());

    let builtin = state.presets_for(repo).unwrap()[0].id.clone();
    assert!(state.remove_preset(&builtin).is_err());
}

#[test]
fn a_damaged_user_preset_does_not_hide_the_catalogue() {
    let dir = tempfile::tempdir().unwrap();
    let f = test_fixtures::linear(1).unwrap();
    let (state, repo) = opened(&f);
    state.use_preset_dir(dir.path().to_path_buf());
    std::fs::write(
        dir.path().join("broken.toml"),
        "this is not toml at all [[[",
    )
    .unwrap();

    assert!(!state.presets_for(repo).unwrap().is_empty());
}
