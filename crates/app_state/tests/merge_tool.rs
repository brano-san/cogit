#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, SystemTime};

use app_state::merge_tool::*;

fn files(root: &Path) -> ToolFiles {
    ToolFiles {
        dir: root.to_path_buf(),
        base: root.join("a.base.cpp"),
        ours: root.join("a.ours.cpp"),
        theirs: root.join("a.theirs.cpp"),
        result: root.join("a.cpp"),
    }
}

fn win_files() -> ToolFiles {
    let dir = PathBuf::from(r"C:\Temp\cogit-merge\1");
    ToolFiles {
        base: dir.join("plate navboard2.base.cpp"),
        ours: dir.join("plate navboard2.ours.cpp"),
        theirs: dir.join("plate navboard2.theirs.cpp"),
        result: PathBuf::from(r"D:\Проекты\мой репо\src/девайс/plate navboard2.cpp"),
        dir,
    }
}

#[test]
fn prepare_names_and_missing_sides() {
    let tmp = tempfile::tempdir().unwrap();
    let f = prepare(
        tmp.path(),
        Path::new("/repo"),
        "src/девайс/plate navboard2.cpp",
        None,
        Some(b"o"),
        Some(b"t"),
    )
    .unwrap();
    assert_eq!(f.base.file_name().unwrap(), "plate navboard2.base.cpp");
    assert_eq!(f.ours.file_name().unwrap(), "plate navboard2.ours.cpp");
    assert_eq!(std::fs::read(&f.base).unwrap(), b"");
    assert_eq!(std::fs::read(&f.ours).unwrap(), b"o");
    assert_eq!(
        f.result,
        Path::new("/repo").join("src/девайс/plate navboard2.cpp")
    );
    assert!(f.dir.join(".owner").exists());
    assert!(f.dir.starts_with(tmp.path().join("cogit-merge")));
    let g = prepare(
        tmp.path(),
        Path::new("/repo"),
        "src/devices/plate navboard2.cpp",
        None,
        None,
        None,
    )
    .unwrap();
    assert_ne!(f.dir, g.dir);
}

#[test]
fn prepare_no_extension_dotfile_and_invalid_chars() {
    let tmp = tempfile::tempdir().unwrap();
    let r = Path::new("/repo");
    let f = prepare(tmp.path(), r, "Makefile", None, None, None).unwrap();
    assert_eq!(f.base.file_name().unwrap(), "Makefile.base");
    let f = prepare(tmp.path(), r, ".gitignore", None, None, None).unwrap();
    assert_eq!(f.theirs.file_name().unwrap(), ".gitignore.theirs");
    let f = prepare(tmp.path(), r, "dir\\a<b>:c?.t*t", None, None, None).unwrap();
    assert_eq!(f.ours.file_name().unwrap(), "a_b__c_.ours.t_t");
}

#[test]
fn cleanup_is_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    let f = prepare(tmp.path(), Path::new("/r"), "a.txt", None, None, None).unwrap();
    f.cleanup();
    assert!(!f.dir.exists());
    f.cleanup();
}

#[test]
fn sweep_removes_only_old_launch_dirs() {
    let tmp = tempfile::tempdir().unwrap();
    let old = prepare(tmp.path(), Path::new("/r"), "a.txt", None, None, None).unwrap();
    let nomarker = tmp.path().join("cogit-merge").join("nomarker");
    std::fs::create_dir_all(&nomarker).unwrap();
    let outside = tmp.path().join("keep");
    std::fs::create_dir_all(&outside).unwrap();
    let age = Duration::from_secs(3600);

    assert_eq!(sweep(tmp.path(), age, SystemTime::now()), 0);
    assert!(old.dir.exists() && nomarker.exists());

    let later = SystemTime::now() + Duration::from_secs(7200);
    assert_eq!(sweep(tmp.path(), age, later), 2);
    assert!(!old.dir.exists() && !nomarker.exists() && outside.exists());
    assert_eq!(sweep(tmp.path(), age, later), 0);
    assert_eq!(sweep(&tmp.path().join("missing"), age, later), 0);
}

#[test]
fn tokenize_rules() {
    let t = |s| tokenize(s).unwrap();
    assert_eq!(t("a  b\tc"), ["a", "b", "c"]);
    assert_eq!(
        t(r#"--out="a b" 'c d' "" x"#),
        ["--out=a b", "c d", "", "x"]
    );
    assert_eq!(t(r#""say \"hi\"""#), [r#"say "hi""#]);
    assert_eq!(
        t(r#""C:\Program Files\Мелд\x.exe" y"#),
        [r"C:\Program Files\Мелд\x.exe", "y"]
    );
    assert_eq!(t(r"C:\a\b D:\c"), [r"C:\a\b", r"D:\c"]);
    assert!(t("").is_empty());
    assert!(matches!(
        tokenize(r#"a "b"#),
        Err(ToolError::UnbalancedQuote)
    ));
    assert!(matches!(tokenize("a 'b"), Err(ToolError::UnbalancedQuote)));
    assert!(matches!(
        tokenize(r#""C:\dir\""#),
        Err(ToolError::UnbalancedQuote)
    ));
}

#[test]
fn substitute_and_brace_escapes() {
    let f = files(Path::new("/t"));
    assert_eq!(
        substitute("x={ours}|{theirs}|{base}|{result}", &f).unwrap(),
        format!(
            "x={}|{}|{}|{}",
            f.ours.display(),
            f.theirs.display(),
            f.base.display(),
            f.result.display()
        )
    );
    assert_eq!(substitute("{{ours}} }}", &f).unwrap(), "{ours} }");
    assert!(
        matches!(substitute("{nope}", &f), Err(ToolError::UnknownPlaceholder(n)) if n == "nope")
    );
    assert!(matches!(
        substitute("{ours", &f),
        Err(ToolError::UnknownPlaceholder(_))
    ));
}

#[test]
fn windows_paths_stay_single_arguments() {
    let f = win_files();
    let c = build_command(
        r#" "C:\Program Files\Beyond Compare 4\BComp.exe" "#,
        r#""{ours}" "{theirs}" "{base}" "{result}""#,
        &f,
    )
    .unwrap();
    assert_eq!(c.program, r"C:\Program Files\Beyond Compare 4\BComp.exe");
    assert_eq!(c.args.len(), 4);
    assert!(c.args[0].ends_with("plate navboard2.ours.cpp"));
    assert!(c.args[1].ends_with("plate navboard2.theirs.cpp"));
    assert!(c.args[2].ends_with("plate navboard2.base.cpp"));
    assert_eq!(c.args[3], f.result.to_string_lossy());
    assert!(c.args[3].starts_with(r"D:\Проекты\мой репо\"));

    let unquoted = build_command("p", "{result} {ours}", &f).unwrap();
    assert_eq!(unquoted.args.len(), 2);
}

#[test]
fn build_command_defaults_and_errors() {
    let f = files(Path::new("/t"));
    let c = build_command("tool", "  ", &f).unwrap();
    assert_eq!(
        c.args,
        [&f.base, &f.ours, &f.theirs, &f.result].map(|p| p.to_string_lossy().into_owned())
    );
    assert!(matches!(
        build_command(" \"\" ", "", &f),
        Err(ToolError::EmptyProgram)
    ));
    assert!(matches!(
        build_command("t", "{x}", &f),
        Err(ToolError::UnknownPlaceholder(_))
    ));
}

fn cfg(tool: Option<&str>, cmd: Option<&str>, path: Option<&str>) -> GitToolConfig {
    let o = |s: Option<&str>| s.map(str::to_owned);
    GitToolConfig {
        tool: o(tool),
        cmd: o(cmd),
        path: o(path),
    }
}

#[test]
fn git_cmd_conversion() {
    let (p, a) = from_git_config(&cfg(
        Some("x"),
        Some(r#""C:\Program Files\M\m.exe" --a "$LOCAL" ${REMOTE} $BASE -o "$MERGED""#),
        None,
    ))
    .unwrap();
    assert_eq!(p, r"C:\Program Files\M\m.exe");
    assert_eq!(a, r#"--a "{ours}" {theirs} {base} -o "{result}""#);
    for bad in ["a && b", "a || b", "a; b", "a | b", "a > b", "a $(b)"] {
        assert!(matches!(
            from_git_config(&cfg(None, Some(bad), None)),
            Err(ToolError::ShellCommandUnsupported(_))
        ));
    }
}

#[test]
fn git_builtin_table() {
    let (p, a) = from_git_config(&cfg(Some("meld"), None, None)).unwrap();
    assert_eq!(
        (p.as_str(), a.as_str()),
        ("meld", "--output {result} {ours} {base} {theirs}")
    );
    let (p, _) = from_git_config(&cfg(Some("vscode"), None, None)).unwrap();
    assert_eq!(p, "code");
    let (p, _) = from_git_config(&cfg(Some("bc3"), None, None)).unwrap();
    assert_eq!(p, "bcomp");
    let (p, a) = from_git_config(&cfg(Some("kdiff3"), None, Some(r"C:\K\kdiff3.exe"))).unwrap();
    assert_eq!(
        (p.as_str(), a.as_str()),
        (r"C:\K\kdiff3.exe", "{base} {ours} {theirs} -o {result}")
    );
    for t in ["vimdiff", "nvimdiff", "emerge", "xxdiff", "zzz"] {
        assert!(matches!(
            from_git_config(&cfg(Some(t), None, None)),
            Err(ToolError::UnknownTool(_))
        ));
    }
    assert!(matches!(
        from_git_config(&cfg(None, None, None)),
        Err(ToolError::NoToolConfigured)
    ));
}

#[test]
fn conflict_markers() {
    assert!(has_conflict_markers(
        "a\n<<<<<<< HEAD\nx\n=======\ny\n>>>>>>> topic\n"
    ));
    assert!(has_conflict_markers("<<<<<<<\r\nx\r\n>>>>>>>\r\n"));
    assert!(!has_conflict_markers("Title\n=======\ntext\n"));
    assert!(!has_conflict_markers(
        "quote: <<<<<<< HEAD only\n<<<<<<< HEAD\n"
    ));
    assert!(!has_conflict_markers(
        "<<<<<<<<< not a marker\n>>>>>>>>> nor this\n"
    ));
}

#[cfg(windows)]
mod child {
    pub const EXIT3: (&str, &str) = ("cmd", "/C exit 3");
    pub const SLEEP: (&str, &str) = ("ping", "-n 30 127.0.0.1");
    pub const REWRITE: (&str, &str) = (
        "powershell",
        "-NoProfile -Command \"Set-Content -Path '{result}' -Value merged\"",
    );
}
#[cfg(unix)]
mod child {
    pub const EXIT3: (&str, &str) = ("sh", "-c 'exit 3'");
    pub const SLEEP: (&str, &str) = ("sleep", "30");
    pub const REWRITE: (&str, &str) = ("sh", "-c \"echo merged > '{result}'\"");
}

struct Harness {
    tmp: tempfile::TempDir,
    tools: MergeTools,
}

impl Harness {
    fn new() -> Self {
        Self {
            tmp: tempfile::tempdir().unwrap(),
            tools: MergeTools::default(),
        }
    }

    fn launch(
        &self,
        path: &str,
        prog: (&str, &str),
    ) -> Result<(ToolKey, ToolFiles, mpsc::Receiver<ToolExit>), ToolError> {
        let repo = self.tmp.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let files = prepare(
            self.tmp.path(),
            &repo,
            path,
            Some(b"b"),
            Some(b"o"),
            Some(b"t"),
        )
        .unwrap();
        let cmd = build_command(prog.0, prog.1, &files)?;
        let key = ToolKey {
            repo: 1,
            path: path.to_owned(),
        };
        let (tx, rx) = mpsc::channel();
        self.tools
            .start(key.clone(), cmd, &repo, files.clone(), move |e| {
                tx.send(e).unwrap();
            })?;
        Ok((key, files, rx))
    }
}

const WAIT: Duration = Duration::from_secs(60);

#[test]
fn exit_code_reported_and_files_cleaned() {
    let h = Harness::new();
    let (key, files, rx) = h.launch("a.txt", child::EXIT3).unwrap();
    let exit = rx.recv_timeout(WAIT).unwrap();
    assert_eq!(
        exit,
        ToolExit {
            code: Some(3),
            canceled: false,
            files_dir_removed: true
        }
    );
    assert!(!files.dir.exists());
    assert!(!h.tools.is_running(&key));
}

#[test]
fn cancel_kills_reports_and_cleans() {
    let h = Harness::new();
    let (key, files, rx) = h.launch("a.txt", child::SLEEP).unwrap();
    assert!(h.tools.is_running(&key));
    assert_eq!(h.tools.running(), vec![key.clone()]);
    assert!(h.tools.cancel(&key));
    let exit = rx.recv_timeout(WAIT).unwrap();
    assert!(exit.canceled && exit.files_dir_removed);
    assert!(!files.dir.exists());
    assert!(!h.tools.cancel(&key));
}

#[test]
fn second_start_for_same_key_is_refused() {
    let h = Harness::new();
    let (key, _files, rx) = h.launch("a.txt", child::SLEEP).unwrap();
    assert!(matches!(
        h.launch("a.txt", child::EXIT3),
        Err(ToolError::AlreadyRunning)
    ));
    assert!(h.tools.cancel(&key));
    rx.recv_timeout(WAIT).unwrap();
}

#[test]
fn tool_rewriting_result_is_observable() {
    let h = Harness::new();
    let (_, files, rx) = h.launch("a.txt", child::REWRITE).unwrap();
    let exit = rx.recv_timeout(WAIT).unwrap();
    assert_eq!(exit.code, Some(0));
    assert_eq!(
        std::fs::read_to_string(&files.result).unwrap().trim(),
        "merged"
    );
}

#[test]
fn missing_program_fails_and_leaves_nothing() {
    let h = Harness::new();
    let r = h.launch("a.txt", ("cogit-no-such-merge-tool-xyz", ""));
    assert!(matches!(r, Err(ToolError::SpawnFailed { .. })));
    let root = h.tmp.path().join("cogit-merge");
    assert_eq!(std::fs::read_dir(root).unwrap().count(), 0);
    assert!(h.tools.running().is_empty());
}
