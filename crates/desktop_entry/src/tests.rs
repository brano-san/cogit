use super::*;

fn os(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

#[test]
fn flags_are_recognized_anywhere() {
    assert_eq!(parse_args(os(&["cogit"])), Action::Run);
    assert_eq!(parse_args(os(&["cogit", "/repo"])), Action::Run);
    assert_eq!(parse_args(os(&["cogit", "--version"])), Action::Version);
    assert_eq!(parse_args(os(&["cogit", "-h"])), Action::Help);
    assert_eq!(
        parse_args(os(&["cogit", "x", "--install-desktop-entry"])),
        Action::Install
    );
    assert_eq!(
        parse_args(os(&["cogit", "--uninstall-desktop-entry"])),
        Action::Uninstall
    );
}

#[test]
fn exec_quoting_follows_the_spec() {
    assert_eq!(exec_arg("/opt/cogit/cogit"), "/opt/cogit/cogit");
    assert_eq!(exec_arg("/my apps/cogit"), "\"/my apps/cogit\"");
    assert_eq!(exec_arg("/100%/cogit"), "/100%%/cogit");
    assert_eq!(exec_arg("/a$b/cogit"), "\"/a\\\\$b/cogit\"");
    assert_eq!(exec_arg("/a\"b"), "\"/a\\\\\"b\"");
}

#[test]
fn desktop_text_is_exact() {
    let text = desktop_file(Path::new("/opt/my apps/cogit"), "Git GUI");
    assert_eq!(
        text,
        "[Desktop Entry]\nType=Application\nName=Cogit\nComment=Git GUI\n\
         Exec=\"/opt/my apps/cogit\" %U\nIcon=cogit\nTerminal=false\nStartupWMClass=cogit\n\
         Categories=Development;RevisionControl;\nStartupNotify=true\n"
    );
}

#[test]
fn data_home_honors_xdg_only_when_absolute() {
    let abs = std::env::temp_dir();
    let abs_os = Some(abs.clone().into_os_string());
    let rel = Some(OsString::from("rel/x"));
    assert_eq!(data_home(abs_os.clone(), rel.clone()), Some(abs.clone()));
    assert_eq!(
        data_home(rel.clone(), abs_os.clone()),
        Some(abs.join(".local/share"))
    );
    assert_eq!(data_home(None, abs_os), Some(abs.join(".local/share")));
    assert_eq!(data_home(None, rel), None);
    assert_eq!(data_home(None, None), None);
}

#[test]
fn install_is_idempotent_and_uninstall_removes_exactly_ours() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let foreign = home.join("icons/hicolor/48x48/apps/other.png");
    std::fs::create_dir_all(foreign.parent().unwrap()).unwrap();
    std::fs::write(&foreign, b"x").unwrap();

    let exe = Path::new("/opt/cogit");
    let first = install(home, exe, "c").unwrap();
    assert_eq!(first.len(), 9);
    assert_eq!(install(home, exe, "c").unwrap(), first);
    let desktop = std::fs::read_to_string(home.join("applications/cogit.desktop")).unwrap();
    assert!(desktop.contains("Exec=/opt/cogit %U"));
    let svg = std::fs::read_to_string(home.join("icons/hicolor/scalable/apps/cogit.svg")).unwrap();
    assert!(svg.starts_with("<svg"));
    let png = std::fs::read(home.join("icons/hicolor/256x256/apps/cogit.png")).unwrap();
    assert!(png.starts_with(b"\x89PNG"));

    let mut removed = uninstall(home).unwrap();
    removed.sort();
    let mut expected = first.clone();
    expected.sort();
    assert_eq!(removed, expected);
    assert!(uninstall(home).unwrap().is_empty());
    assert!(first.iter().all(|p| !p.exists()));
    assert!(foreign.exists());
}

#[test]
fn version_and_help_print_and_run_returns_none() {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(
        handle(os(&["cogit"]), "1.2.3", "c", &mut out, &mut err),
        None
    );
    assert_eq!(
        handle(
            os(&["cogit", "--version"]),
            "1.2.3",
            "c",
            &mut out,
            &mut err
        ),
        Some(0)
    );
    assert_eq!(String::from_utf8(out.clone()).unwrap(), "cogit 1.2.3\n");
    assert_eq!(
        handle(os(&["cogit", "--help"]), "1.2.3", "c", &mut out, &mut err),
        Some(0)
    );
    assert!(
        String::from_utf8(out)
            .unwrap()
            .contains("--install-desktop-entry")
    );
    assert!(err.is_empty());
}
