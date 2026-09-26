// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Repository ▸ Edit Git Config: the path comes from git, the text is checked by git
//! before it is written, and a file that fails the check is left as it was (R-155).

use git_engine::{
    ConfigProblem, GitError, RepoHandle, origin_file, read_config, save_config,
    user_config_by_rules,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn same(a: &Path, b: &Path) -> bool {
    std::fs::canonicalize(a).unwrap() == std::fs::canonicalize(b).unwrap()
}

#[test]
fn a_repositorys_config_is_the_one_in_its_git_directory() {
    let f = test_fixtures::linear(1).unwrap();
    let path = RepoHandle::open(f.path()).unwrap().config_path().unwrap();
    assert!(same(&path, &f.git_dir().join("config")));
}

/// A submodule keeps its config in the parent's `.git/modules`, which is why the path has
/// to come from git rather than from `<root>/.git/config`.
#[test]
fn a_submodules_config_is_under_the_parents_modules() {
    let f = test_fixtures::with_submodule().unwrap();
    let inner = RepoHandle::open_exact(&f.path().join("vendor/lib")).unwrap();
    let path = inner.config_path().unwrap();
    assert!(same(&path, &f.git_dir().join("modules/vendor/lib/config")));
}

#[test]
fn a_linked_worktree_edits_the_config_it_shares() {
    let f = test_fixtures::with_worktree().unwrap();
    let admin = std::fs::read_dir(f.git_dir().join("worktrees"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let gitdir = std::fs::read_to_string(admin.join("gitdir")).unwrap();
    let linked = Path::new(gitdir.trim()).parent().unwrap().to_path_buf();

    let path = RepoHandle::open(&linked).unwrap().config_path().unwrap();
    assert!(same(&path, &f.git_dir().join("config")));
}

#[test]
fn valid_text_is_saved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config");
    save_config(&path, "[core]\n\tbare = false\n", false).unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "[core]\n\tbare = false\n"
    );
}

#[test]
fn text_git_rejects_names_the_line_and_is_not_saved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config");
    std::fs::write(&path, "[core]\n\tbare = false\n").unwrap();

    let result = save_config(&path, "[core]\n\tbare = false\n[broken\n", false);

    match result {
        Err(GitError::ConfigInvalid(ConfigProblem { line, .. })) => assert_eq!(line, Some(3)),
        other => panic!("expected a rejected config, got {other:?}"),
    }
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "[core]\n\tbare = false\n",
        "the file on disk is untouched"
    );
    let leftovers: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(leftovers.len(), 1, "the checked copy is removed");
}

// Git writes a config through `config.lock` and renames it over; a save that went past the
// lock replaced the file under a `git config` in the middle of writing it, and one of the
// two changes was lost.
#[test]
fn a_config_git_is_writing_is_left_to_git() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config");
    std::fs::write(&path, "[core]\n\tbare = false\n").unwrap();
    let lock = dir.path().join("config.lock");
    std::fs::write(&lock, "[core]\n\tbare = false\n[x]\n\ty = 1\n").unwrap();

    let result = save_config(&path, "[core]\n\tbare = true\n", false);

    assert!(result.is_err(), "{result:?}");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "[core]\n\tbare = false\n"
    );
    assert!(
        std::fs::read_to_string(&lock).unwrap().contains("[x]"),
        "git's lock is git's to finish"
    );
}

#[test]
fn a_save_takes_the_lock_git_takes_and_lets_it_go() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config");

    save_config(&path, "[core]\n\tbare = true\n", false).unwrap();

    let left: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(left, ["config"]);
}

// A `git.exe` reading the config holds it without FILE_SHARE_DELETE, and Windows then
// refuses to replace it: "Access is denied (os error 5)" for a moment's overlap.
#[cfg(windows)]
#[test]
fn a_config_another_process_is_reading_is_saved_once_it_lets_go() {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_SHARE_READ: u32 = 1;
    const FILE_SHARE_WRITE: u32 = 2;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config");
    std::fs::write(&path, "[core]\n\tbare = false\n").unwrap();
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(&path)
        .unwrap();
    let reader = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        drop(held);
    });

    let result = save_config(&path, "[core]\n\tbare = true\n", false);
    reader.join().unwrap();

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "[core]\n\tbare = true\n"
    );
}

#[test]
fn a_missing_user_config_is_created_with_its_folder() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("xdg/git/config");
    save_config(&path, "[user]\n\tname = Ada\n", false).unwrap();
    assert!(path.is_file());
}

/// CLAUDE.md: text is edited as `\n`, and written back with the ending it came with.
#[test]
fn a_file_with_windows_line_endings_keeps_them() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config");
    std::fs::write(&path, "[core]\r\n\tbare = false\r\n").unwrap();

    let file = read_config(&path).unwrap();
    assert_eq!(file.text, "[core]\n\tbare = false\n");
    assert!(file.crlf);

    save_config(&path, "[core]\n\tbare = true\n", file.crlf).unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "[core]\r\n\tbare = true\r\n"
    );
}

#[test]
fn a_config_that_does_not_exist_yet_reads_as_empty() {
    let dir = tempfile::tempdir().unwrap();
    let file = read_config(&dir.path().join("config")).unwrap();
    assert_eq!(file.text, "");
    assert!(!file.exists);
}

#[test]
fn the_origin_of_the_first_entry_is_the_file_git_reads() {
    let listing = "file:C:/Users/ada/.gitconfig\tuser.name=Ada\nfile:C:/Users/ada/.gitconfig\tuser.email=a@b\n";
    assert_eq!(
        origin_file(listing),
        Some(PathBuf::from("C:/Users/ada/.gitconfig"))
    );
    assert_eq!(origin_file(""), None);
}

fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
fn git_config_global_wins_over_everything() {
    let vars = env(&[("GIT_CONFIG_GLOBAL", "D:/cfg/global"), ("HOME", "C:/h")]);
    let path = user_config_by_rules(&|name| vars.get(name).cloned(), &|_| true);
    assert_eq!(path, PathBuf::from("D:/cfg/global"));
}

#[test]
fn the_home_gitconfig_is_used_when_it_exists() {
    let vars = env(&[("HOME", "C:/h"), ("XDG_CONFIG_HOME", "C:/x")]);
    let path = user_config_by_rules(&|name| vars.get(name).cloned(), &|p| {
        p == Path::new("C:/h/.gitconfig")
    });
    assert_eq!(path, PathBuf::from("C:/h/.gitconfig"));
}

#[test]
fn the_xdg_file_is_used_when_only_it_exists() {
    let vars = env(&[("HOME", "C:/h"), ("XDG_CONFIG_HOME", "C:/x")]);
    let path = user_config_by_rules(&|name| vars.get(name).cloned(), &|p| {
        p == Path::new("C:/x/git/config")
    });
    assert_eq!(path, PathBuf::from("C:/x/git/config"));
}

#[test]
fn with_neither_file_the_home_gitconfig_is_where_one_is_made() {
    let vars = env(&[("HOME", "C:/h")]);
    let path = user_config_by_rules(&|name| vars.get(name).cloned(), &|_| false);
    assert_eq!(path, PathBuf::from("C:/h/.gitconfig"));
}

#[test]
fn without_home_windows_falls_back_to_the_user_profile() {
    let vars = env(&[("USERPROFILE", "C:/Users/ada")]);
    let path = user_config_by_rules(&|name| vars.get(name).cloned(), &|_| false);
    assert_eq!(path, PathBuf::from("C:/Users/ada/.gitconfig"));
}
