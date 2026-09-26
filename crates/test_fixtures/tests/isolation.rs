// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used)]

//! A test run from a hook or a terminal inside one inherits `GIT_DIR` and `GIT_INDEX_FILE`;
//! a `git` spawned past the fixtures then clones, fetches or commits into that repository.

use std::path::Path;

fn sources_under(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources_under(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn tests_spawn_git_only_through_the_fixtures() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut files = Vec::new();
    for entry in std::fs::read_dir(crates).unwrap() {
        let tests = entry.unwrap().path().join("tests");
        if tests.is_dir() {
            sources_under(&tests, &mut files);
        }
    }
    assert!(files.len() > 100, "found only {} test files", files.len());

    let needle = ["Command::new(", "\"git\")"].concat();
    let raw: Vec<String> = files
        .iter()
        .filter(|file| std::fs::read_to_string(file).unwrap().contains(&needle))
        .map(|file| file.display().to_string())
        .collect();

    assert!(
        raw.is_empty(),
        "use test_fixtures::git_command_in or user_git_command instead: {raw:?}"
    );
}

// With `core.quotepath = false` the suite parsed git's output in a mode no user has: a
// split-off that did not recognise a quoted Cyrillic path passed every test (1493aa7).
#[test]
fn the_fixtures_quote_paths_as_git_does_by_default() {
    let f = test_fixtures::unicode_paths().unwrap();

    let listed = f.git(&["ls-files"]).unwrap();

    assert!(
        listed
            .lines()
            .any(|line| line.starts_with('"') && line.contains(char::from(92))),
        "{listed}"
    );
}
