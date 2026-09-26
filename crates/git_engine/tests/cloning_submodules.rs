//! Include submodules. A submodule on the same disk is a local path, which git refuses by
//! default (R-442); only this binary's global config lifts it, so it has one test.
#![allow(unsafe_code, clippy::unwrap_used)]

use git_engine::{CloneRequest, NetworkStop, clone_repository};

#[test]
fn a_clone_with_submodules_checks_them_out() {
    let config = tempfile::tempdir().unwrap();
    let global = config.path().join("gitconfig");
    std::fs::write(&global, "[protocol \"file\"]\n\tallow = always\n").unwrap();
    // SAFETY: the only thread of this test binary that touches the environment.
    unsafe {
        std::env::set_var("GIT_CONFIG_GLOBAL", &global);
    }
    let f = test_fixtures::with_submodule().unwrap();
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("app");
    let request = CloneRequest {
        source: f.path().to_string_lossy().into_owned(),
        target: target.to_string_lossy().into_owned(),
        submodules: true,
        all_branches: true,
        branch: None,
        skip_larger_than_mb: None,
    };

    clone_repository(&request, None, &NetworkStop::default(), None, |_| {}).unwrap();

    let out = test_fixtures::git_command_in(&target)
        .args(["submodule", "status"])
        .output()
        .unwrap();
    let status = String::from_utf8_lossy(&out.stdout);
    assert!(
        status.starts_with(' '),
        "initialized and checked out: {status}"
    );
    assert!(
        std::fs::read_dir(target.join("vendor/lib"))
            .unwrap()
            .count()
            > 1,
        "the submodule has its files"
    );
}
