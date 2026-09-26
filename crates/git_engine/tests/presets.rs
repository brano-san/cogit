// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git_engine::{RepoHandle, builtin_presets, parse_preset};

fn open(f: &test_fixtures::Fixture) -> RepoHandle {
    RepoHandle::open(f.path()).unwrap()
}

#[test]
fn a_preset_is_read_out_of_its_toml() {
    let text = r##"
id = "demo"
name = "Demo"
hook = "pre-commit"
description = "Does nothing."
config_files = []
slow = false
script = "#!/bin/sh\nexit 0\n"
"##;

    let preset = parse_preset(text).unwrap();

    assert_eq!(preset.id, "demo");
    assert_eq!(preset.hook, "pre-commit");
    assert!(preset.tool.is_none());
}

#[test]
fn a_preset_can_name_the_tool_it_needs() {
    let text = r##"
id = "demo"
name = "Demo"
hook = "pre-commit"
description = "Needs cargo."
config_files = ["rustfmt.toml"]
slow = false
script = "#!/bin/sh\nexit 0\n"

[tool]
command = "cargo"
install_hint = "Install Rust."
search_paths = ["$CARGO_HOME/bin"]
"##;

    let tool = parse_preset(text).unwrap().tool.unwrap();

    assert_eq!(tool.command, "cargo");
    assert!(!tool.install_hint.is_empty());
    assert_eq!(tool.search_paths, vec!["$CARGO_HOME/bin".to_owned()]);
}

#[test]
fn toml_that_is_not_a_preset_is_a_typed_error() {
    assert!(parse_preset("this is not toml at all [[[").is_err());
    assert!(parse_preset("id = \"only-an-id\"").is_err());
}

#[test]
fn the_catalogue_ships_with_presets() {
    assert!(builtin_presets().len() >= 4);
}

#[test]
fn every_builtin_preset_targets_a_real_git_hook() {
    for preset in builtin_presets() {
        assert!(
            git_engine::is_hook_name(&preset.hook),
            "{} targets {}",
            preset.id,
            preset.hook
        );
    }
}

/// The seven requirements of T10.4 are acceptance criteria, so they are a test.
#[test]
fn no_builtin_preset_stages_work_the_author_did_not_choose() {
    for preset in builtin_presets() {
        // What the script runs, not what its comments mention.
        let commands: Vec<&str> = preset.script.lines().map(str::trim).collect();
        assert!(
            !commands
                .iter()
                .any(|line| line.starts_with("git add -A") || line.starts_with("git add .")),
            "{} stages everything",
            preset.id
        );
    }
}

#[test]
fn every_builtin_preset_looks_only_at_staged_files() {
    for preset in builtin_presets() {
        if preset.hook != "pre-commit" {
            continue;
        }
        assert!(
            preset.script.contains("--cached"),
            "{} does not limit itself to the index",
            preset.id
        );
    }
}

#[test]
fn every_builtin_preset_is_written_with_unix_line_endings() {
    for preset in builtin_presets() {
        assert!(
            !preset.script.contains('\r'),
            "{} has CRLF, which breaks the shebang on Windows",
            preset.id
        );
    }
}

#[test]
fn every_builtin_preset_starts_with_a_shebang() {
    for preset in builtin_presets() {
        assert!(
            preset.script.trim_start().starts_with("#!/bin/sh"),
            "{} has no shebang",
            preset.id
        );
    }
}

#[test]
fn a_preset_that_needs_a_tool_says_how_to_install_it() {
    for preset in builtin_presets() {
        if let Some(tool) = &preset.tool {
            assert!(!tool.install_hint.is_empty(), "{} gives no hint", preset.id);
            assert!(
                !tool.search_paths.is_empty(),
                "{} relies on the caller's PATH alone",
                preset.id
            );
        }
    }
}

#[test]
fn a_preset_that_needs_a_tool_looks_for_it_before_running() {
    for preset in builtin_presets() {
        let Some(tool) = &preset.tool else { continue };
        assert!(
            preset.script.contains(&tool.command),
            "{} never mentions {}",
            preset.id,
            tool.command
        );
        assert!(
            preset.script.contains("command -v") || preset.script.contains("-x "),
            "{} does not look for its tool",
            preset.id
        );
    }
}

#[test]
fn a_formatting_preset_refuses_to_rewrite_a_partially_staged_file() {
    let rust = builtin_presets()
        .into_iter()
        .find(|preset| preset.id == "rust-fmt")
        .unwrap();

    assert!(
        rust.script.contains("git diff --name-only"),
        "it must notice a file that is staged only in part"
    );
}

#[test]
fn every_preset_has_a_unique_id() {
    let mut ids: Vec<String> = builtin_presets().into_iter().map(|p| p.id).collect();
    let before = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), before);
}

#[test]
fn a_tool_on_the_path_is_found() {
    let tool = git_engine::PresetTool {
        command: "git".to_owned(),
        install_hint: "Install Git.".to_owned(),
        search_paths: Vec::new(),
    };

    assert!(git_engine::find_tool(&tool).is_some());
}

#[test]
fn a_tool_that_is_nowhere_is_reported_missing() {
    let tool = git_engine::PresetTool {
        command: "definitely-not-installed-xyz".to_owned(),
        install_hint: "You cannot.".to_owned(),
        search_paths: Vec::new(),
    };

    assert!(git_engine::find_tool(&tool).is_none());
}

#[test]
fn a_declared_directory_is_searched_before_the_path() {
    let dir = tempfile::tempdir().unwrap();
    let name = if cfg!(windows) {
        "planted.exe"
    } else {
        "planted"
    };
    let planted = dir.path().join(name);
    std::fs::write(&planted, b"#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&planted, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let tool = git_engine::PresetTool {
        command: "planted".to_owned(),
        install_hint: "n/a".to_owned(),
        search_paths: vec![dir.path().to_string_lossy().into_owned()],
    };

    assert_eq!(
        git_engine::find_tool(&tool).as_deref(),
        Some(planted.as_path())
    );
}

#[test]
fn an_environment_variable_in_a_search_path_is_expanded() {
    let dir = tempfile::tempdir().unwrap();
    let tool = git_engine::PresetTool {
        command: "nothing-here".to_owned(),
        install_hint: "n/a".to_owned(),
        search_paths: vec!["$COGIT_TEST_UNSET_VAR/bin".to_owned()],
    };

    // An unset variable must not become the literal path "$COGIT_TEST_UNSET_VAR/bin".
    assert!(git_engine::find_tool(&tool).is_none());
    drop(dir);
}

#[test]
fn installing_a_preset_writes_its_hook() {
    let f = test_fixtures::linear(1).unwrap();
    let preset = builtin_presets()
        .into_iter()
        .find(|p| p.id == "commit-message")
        .unwrap();

    open(&f).install_preset(&preset).unwrap();

    let body = open(&f).read_hook("commit-msg").unwrap();
    assert!(body.contains("type(scope)"), "{body}");
    assert!(!body.contains('\r'), "the hook must be LF only");
}

#[test]
fn installing_a_preset_makes_the_hook_executable() {
    let f = test_fixtures::linear(1).unwrap();
    let preset = builtin_presets()
        .into_iter()
        .find(|p| p.id == "large-files")
        .unwrap();

    open(&f).install_preset(&preset).unwrap();

    let hook = open(&f)
        .hooks()
        .unwrap()
        .hooks
        .into_iter()
        .find(|h| h.name == "pre-commit")
        .unwrap();
    assert!(hook.executable);
}

// "<tool> not found" alone left the user guessing where Cogit had looked (F-107).
#[test]
fn a_missing_tool_names_every_place_that_was_searched() {
    let dir = tempfile::tempdir().unwrap();
    let tool = git_engine::PresetTool {
        command: "nowhere-to-be-found".to_owned(),
        install_hint: "n/a".to_owned(),
        search_paths: vec![
            dir.path().to_string_lossy().into_owned(),
            "$COGIT_SURELY_UNSET_VARIABLE/bin".to_owned(),
        ],
    };

    assert_eq!(git_engine::find_tool(&tool), None);
    assert_eq!(
        git_engine::tool_search_places(&tool),
        vec![dir.path().to_string_lossy().into_owned(), "PATH".to_owned()],
        "declared directories as expanded, one with an unset variable skipped, then PATH"
    );
}
