// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! A file without a final newline, staged and discarded whole, checked by `git apply`
//! itself: the diff shows the missing newline SmartGit's way, as an empty row, and the
//! patch has to say it git's way.

use diff_engine::{
    DiffOptions, DiffRow, FileDiff, PatchRequest, PatchShape, PatchSides, build_patch, diff_text,
};

const CASES: &[(&str, &str)] = &[
    ("x\n", "x\ny"),
    ("a\n", "a\n\nc"),
    ("a\nb\nc", "a\nb"),
    ("a\nb", "a\nb\nc"),
    ("a\nb\n", "a\nb"),
    ("a\nb", "a\nb\n"),
    ("a\nb\n", "a\nc"),
    ("a\nb", "a\nc\n"),
    ("a\nb\nc\n", "a\nb"),
    ("a\nb", "a\nb\nc\n"),
    ("a\n\n", "a\n\nc"),
    ("1\n2\n3\n4\n5\n6\n7\n8", "1\nTWO\n3\n4\n5\n6\n7\n8"),
];

fn apply(base: &str, patch: &str, reverse: bool) -> Result<String, String> {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f"), base).unwrap();
    std::fs::write(dir.path().join("p.diff"), patch).unwrap();
    let mut git = test_fixtures::git_command_in(dir.path());
    git.args([
        "-c",
        "core.autocrlf=false",
        "apply",
        "--unidiff-zero",
        "--whitespace=nowarn",
    ]);
    if reverse {
        git.arg("-R");
    }
    let out = git.arg("p.diff").output().unwrap();
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(std::fs::read_to_string(dir.path().join("f")).unwrap())
}

/// Every row selected, as Stage Hunk (or Discard Hunk, `reverse`) asks for.
fn whole(old: &str, new: &str, context: u32, reverse: bool) -> Result<String, String> {
    let options = DiffOptions {
        context_lines: context,
        ..DiffOptions::default()
    };
    let FileDiff::Text { hunks, .. } = diff_text(old, new, &options) else {
        return Err("not a text diff".to_owned());
    };
    let rows = || hunks.iter().flat_map(|hunk| hunk.rows.iter());
    let request = PatchRequest {
        path: "f".to_owned(),
        hunks: hunks.clone(),
        selected_deletes: rows()
            .filter_map(|row| match row {
                DiffRow::Delete { old, .. } => Some(*old),
                _ => None,
            })
            .collect(),
        selected_inserts: rows()
            .filter_map(|row| match row {
                DiffRow::Insert { new, .. } => Some(*new),
                _ => None,
            })
            .collect(),
    };
    let shape = PatchShape {
        reverse,
        old_exists: true,
        new_exists: true,
        new_mode: None,
    };
    let sides = PatchSides {
        old: old.as_bytes(),
        new: new.as_bytes(),
    };
    let patch = build_patch(&request, shape, sides).map_err(|err| format!("{err:?}"))?;
    let (base, want) = if reverse { (new, old) } else { (old, new) };
    let got = apply(base, &patch, reverse).map_err(|err| format!("{err}\n{patch}"))?;
    if got == want {
        Ok(got)
    } else {
        Err(format!("got {got:?}, want {want:?}\n{patch}"))
    }
}

#[test]
fn a_whole_change_at_an_open_end_stages_and_discards_as_git_has_it() {
    let mut failures = Vec::new();
    for &(old, new) in CASES {
        for context in [3, 0] {
            for reverse in [false, true] {
                if let Err(err) = whole(old, new, context, reverse) {
                    failures.push(format!(
                        "{old:?} -> {new:?}, context {context}, reverse {reverse}: {err}"
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
