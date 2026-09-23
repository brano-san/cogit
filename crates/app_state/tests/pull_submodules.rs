// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Repository Settings ▸ Fetch and Pull ▸ Initialize new submodules (#42).

use app_state::AppState;
use std::path::{Path, PathBuf};

fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// `mine` is one commit behind `origin`, and that commit adds `vendor/lib`.
struct Scene {
    _dir: tempfile::TempDir,
    _lib: test_fixtures::Fixture,
    seed: test_fixtures::Fixture,
    mine: PathBuf,
}

fn scene() -> Scene {
    let seed = test_fixtures::linear(1).unwrap();
    let lib = test_fixtures::linear(2).unwrap();
    let dir = tempfile::TempDir::new().unwrap();
    let origin = slashed(&dir.path().join("origin.git"));
    let mine = dir.path().join("mine");
    let theirs = dir.path().join("theirs");

    seed.git(&["clone", "--bare", "--", &slashed(seed.path()), &origin])
        .unwrap();
    seed.git(&["clone", "--", &origin, &slashed(&mine)])
        .unwrap();
    seed.git(&["clone", "--", &origin, &slashed(&theirs)])
        .unwrap();
    seed.git_in(
        &theirs,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "--",
            &slashed(lib.path()),
            "vendor/lib",
        ],
    )
    .unwrap();
    seed.git_in(&theirs, &["commit", "-m", "add vendor/lib"])
        .unwrap();
    seed.git_in(&theirs, &["push"]).unwrap();

    Scene {
        _dir: dir,
        _lib: lib,
        seed,
        mine,
    }
}

fn pull(scene: &Scene) {
    let state = AppState::new();
    let repo = state.open_repository(&scene.mine).unwrap().repo;
    state.pull(repo, "origin", true, |_| {}).unwrap();
}

#[test]
fn a_submodule_the_pull_brings_is_checked_out_when_the_repository_asks_for_it() {
    let scene = scene();
    scene
        .seed
        .git_in(&scene.mine, &["config", "cogit.initNewSubmodules", "true"])
        .unwrap();

    pull(&scene);

    assert!(scene.mine.join("vendor/lib/file1.txt").is_file());
}

#[test]
fn without_the_setting_the_pull_leaves_it_as_git_does() {
    let scene = scene();

    pull(&scene);

    assert!(
        scene.mine.join(".gitmodules").is_file(),
        "the pull did not happen"
    );
    assert!(!scene.mine.join("vendor/lib/file1.txt").exists());
}
