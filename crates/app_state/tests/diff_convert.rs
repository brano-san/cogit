// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::AppState;
use diff_engine::{DiffOptions, FileDiff};
use git_engine::DiffSpec;

fn converted_lines(diff: &FileDiff) -> (Option<String>, String) {
    match diff {
        FileDiff::Text {
            hunks, converted, ..
        } => (converted.clone(), serde_json::to_string(hunks).unwrap()),
        other => panic!("expected a text diff, got {other:?}"),
    }
}

// git diff runs `diff.<driver>.textconv` on each side; the lines are its output, and the
// diff says so, since they cannot be staged.
#[test]
fn a_textconv_driver_turns_both_sides_into_its_output() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file(".gitattributes", "*.dat diff=upper\n")
        .unwrap();
    f.write_file("a.dat", "one\n").unwrap();
    f.git(&["add", "."]).unwrap();
    f.commit_staged(5, "data").unwrap();
    f.git(&["config", "diff.upper.textconv", "tr a-z A-Z <"])
        .unwrap();
    f.write_file("a.dat", "two\n").unwrap();

    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let diff = state
        .diff_file(
            repo,
            &DiffSpec::WorkTreeVsIndex,
            "a.dat",
            &DiffOptions::default(),
        )
        .unwrap();

    let (converted, lines) = converted_lines(&diff);
    assert_eq!(converted.as_deref(), Some("textconv: tr a-z A-Z <"));
    assert!(lines.contains("TWO"), "{lines}");
}

#[test]
fn a_utf16_file_with_a_bom_is_diffed_as_text() {
    let f = test_fixtures::linear(1).unwrap();
    let utf16 = |text: &str| {
        let mut bytes = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes
    };
    std::fs::write(f.path().join("wide.txt"), utf16("alpha\n")).unwrap();
    f.git(&["add", "."]).unwrap();
    f.commit_staged(5, "wide").unwrap();
    std::fs::write(f.path().join("wide.txt"), utf16("beta\n")).unwrap();

    let state = AppState::new();
    let repo = state.open_repository(f.path()).unwrap().repo;
    let diff = state
        .diff_file(
            repo,
            &DiffSpec::WorkTreeVsIndex,
            "wide.txt",
            &DiffOptions::default(),
        )
        .unwrap();

    let (converted, lines) = converted_lines(&diff);
    assert_eq!(converted.as_deref(), Some("decoded from UTF-16LE"));
    assert!(lines.contains("beta"), "{lines}");
}
