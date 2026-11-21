#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Open in External Diff Tool and Open in External Editor (F-944): the commands built from
//! Preferences, and the diff tool run end to end through git's shell.

use app_state::AppState;
use app_state::desktop::Platform;
use app_state::external::{ExternalError, diff_tool_shell_command, editor_command};
use git_engine::DiffSpec;
use std::time::{Duration, Instant};

#[test]
fn the_diff_tool_gets_one_quoted_word_per_argument_and_gits_variables() {
    let cmd = diff_tool_shell_command(
        Platform::Windows,
        r"C:\Program Files\Meld\Meld.exe",
        "--label \"it's left\" --left={local} {remote}",
    )
    .unwrap();
    assert_eq!(
        cmd,
        r#"'C:/Program Files/Meld/Meld.exe' '--label' 'it'\''s left' '--left='"$LOCAL" "$REMOTE""#
    );
}

#[test]
fn empty_arguments_compare_local_with_remote() {
    let cmd = diff_tool_shell_command(Platform::Linux, "\"meld\"", "  ").unwrap();
    assert_eq!(cmd, r#"'meld' "$LOCAL" "$REMOTE""#);
}

#[test]
fn a_placeholder_of_the_merge_tool_is_named_in_the_refusal() {
    let err = diff_tool_shell_command(Platform::Linux, "meld", "{base} {remote}").unwrap_err();
    assert!(matches!(&err, ExternalError::UnknownPlaceholder { name, .. } if name == "{base}"));
    assert!(err.to_string().contains("{local}"));
}

#[test]
fn the_editor_gets_the_native_path_and_the_line() {
    let launch = editor_command(
        Platform::Windows,
        r"C:\Tools\Code.exe",
        "--goto \"${file}:${line}\"",
        "C:/repo dir/src/main.rs",
        Some(42),
    )
    .unwrap();
    assert_eq!(launch.program, r"C:\Tools\Code.exe");
    assert_eq!(launch.args, [r"--goto", r"C:\repo dir\src\main.rs:42"]);
}

#[test]
fn the_editor_without_arguments_opens_the_file_at_the_first_line() {
    let launch = editor_command(Platform::Linux, "gedit", "", "/r/a b.txt", None).unwrap();
    assert_eq!(launch.args, ["/r/a b.txt"]);
    let launch = editor_command(Platform::Linux, "vi", "+${line} ${file}", "/r/a", None).unwrap();
    assert_eq!(launch.args, ["+1", "/r/a"]);
}

#[test]
fn mac_application_bundles_open_documents_and_cli_editors_keep_their_arguments() {
    let file = "/r/it's my ${line}.rs";
    let bundle = "/Applications/Visual Studio Code.app";
    let launch = editor_command(Platform::MacOs, bundle, "", file, None).unwrap();
    assert_eq!(launch.program, "open");
    assert_eq!(launch.args, ["-a", bundle, file]);
    let custom = editor_command(
        Platform::MacOs,
        bundle,
        "--goto \"${file}:${line}\"",
        file,
        Some(7),
    )
    .unwrap();
    assert_eq!(
        custom.args,
        [
            "-n",
            "-a",
            bundle,
            "--args",
            "--goto",
            "/r/it's my ${line}.rs:7"
        ]
    );
    let cli = editor_command(
        Platform::MacOs,
        "code",
        "--goto \"${file}:${line}\"",
        file,
        Some(7),
    )
    .unwrap();
    assert_eq!(cli.program, "code");
    assert_eq!(cli.args, ["--goto", "/r/it's my ${line}.rs:7"]);
}

#[test]
fn the_editor_refuses_what_it_cannot_fill_in() {
    assert_eq!(
        editor_command(Platform::Linux, "  ", "", "/r/a", None),
        Err(ExternalError::EmptyProgram { tool: "editor" })
    );
    let err = editor_command(Platform::Linux, "x", "${file} ${column}", "/r/a", None).unwrap_err();
    assert!(matches!(&err, ExternalError::UnknownPlaceholder { name, .. } if name == "${column}"));
    assert!(matches!(
        editor_command(Platform::Linux, "x", "\"${file}", "/r/a", None),
        Err(ExternalError::UnbalancedQuote { .. })
    ));
}

#[test]
fn a_program_from_preferences_receives_both_sides_through_gits_shell() {
    let f = test_fixtures::linear(1).unwrap();
    f.git(&["config", "diff.tool", ""]).unwrap();
    f.git(&["config", "merge.tool", ""]).unwrap();
    std::fs::write(f.path().join("file0.txt"), "on disk\n").unwrap();
    let out = f.git_dir().join("seen");
    let at = out.to_string_lossy().replace('\\', "/");
    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;

    state
        .start_diff_tool(
            repo,
            &DiffSpec::WorkTreeVsIndex,
            "file0.txt",
            None,
            "sh",
            &format!("-c \"cat \\\"$1\\\" \\\"$2\\\" > '{at}.tmp' && mv '{at}.tmp' '{at}'\" sh {{local}} {{remote}}"),
        )
        .unwrap();

    let started = Instant::now();
    while !out.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "the tool never ran"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "content 0\non disk\n"
    );
}
