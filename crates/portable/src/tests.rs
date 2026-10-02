#![allow(clippy::unwrap_used)]

use super::*;
use std::collections::HashMap;

fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
    let map: HashMap<String, OsString> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), OsString::from(v)))
        .collect();
    move |name| map.get(name).cloned()
}

fn set_value(plan: &Redirect, name: &str) -> Option<OsString> {
    plan.set
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, v)| v.clone())
}

#[test]
fn the_binary_folder_is_the_executables_own() {
    let exe = Path::new("/opt/cogit/cogit");
    assert_eq!(binary_folder(exe, None), Some(PathBuf::from("/opt/cogit")));
}

#[test]
fn an_appimage_uses_the_folder_of_the_appimage_not_the_squashfs_mount() {
    let exe = Path::new("/tmp/.mount_CogitAbc/usr/bin/cogit");
    let image = Path::new("/home/ann/apps/Cogit.AppImage");
    assert_eq!(
        binary_folder(exe, Some(image)),
        Some(PathBuf::from("/home/ann/apps"))
    );
}

#[test]
fn an_empty_appimage_variable_falls_back_to_the_executable() {
    let exe = Path::new("/opt/cogit/cogit");
    assert_eq!(
        binary_folder(exe, Some(Path::new(""))),
        Some(PathBuf::from("/opt/cogit"))
    );
}

#[test]
fn a_path_with_spaces_and_cyrillic_keeps_them() {
    let exe = Path::new("D:/Мои программы/Cogit portable/cogit.exe");
    let folder = binary_folder(exe, None).unwrap();
    assert_eq!(folder, PathBuf::from("D:/Мои программы/Cogit portable"));
    let layout = Layout::in_folder(&folder);
    assert_eq!(
        layout.config(),
        PathBuf::from("D:/Мои программы/Cogit portable/Cogit-data/config")
    );
}

#[test]
fn a_bare_file_name_has_no_folder() {
    assert_eq!(binary_folder(Path::new("cogit"), None), None);
}

#[test]
fn the_verbatim_prefix_is_dropped_but_unc_stays() {
    assert_eq!(
        strip_verbatim(Path::new(r"\\?\C:\Users\Аня\Cogit")),
        PathBuf::from(r"C:\Users\Аня\Cogit")
    );
    assert_eq!(
        strip_verbatim(Path::new(r"\\?\UNC\server\share")),
        PathBuf::from(r"\\?\UNC\server\share")
    );
    assert_eq!(
        strip_verbatim(Path::new("/opt/cogit")),
        PathBuf::from("/opt/cogit")
    );
    assert_eq!(
        strip_verbatim(Path::new(r"C:\Users\Cogit")),
        PathBuf::from(r"C:\Users\Cogit")
    );
    let device = r"\\?\Volume{0b6c2a51-0000-0000-0000-100000000000}\Cogit";
    assert_eq!(strip_verbatim(Path::new(device)), PathBuf::from(device));
}

#[test]
fn a_real_path_is_resolved_and_an_unresolvable_one_kept_without_the_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let real = real_path(dir.path());
    assert!(!real.to_string_lossy().starts_with(r"\\?\"), "{real:?}");
    assert_eq!(real, strip_verbatim(&dir.path().canonicalize().unwrap()));

    let missing = dir.path().join("missing");
    assert_eq!(real_path(&missing), missing);
    assert_eq!(
        real_path(Path::new(r"\\?\C:\cogit-missing\x")),
        PathBuf::from(r"C:\cogit-missing\x")
    );
}

#[cfg(unix)]
#[test]
fn a_symlinked_executable_resolves_to_the_real_folder() {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real");
    let links = dir.path().join("links");
    std::fs::create_dir_all(&real).unwrap();
    std::fs::create_dir_all(&links).unwrap();
    std::fs::write(real.join("cogit"), b"").unwrap();
    std::os::unix::fs::symlink(real.join("cogit"), links.join("cogit")).unwrap();

    let resolved = links.join("cogit").canonicalize().unwrap();
    assert_eq!(
        binary_folder(&resolved, None),
        Some(real.canonicalize().unwrap())
    );
}

#[test]
fn ensure_creates_every_folder_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let layout = Layout::in_folder(dir.path());
    layout.ensure().unwrap();
    std::fs::write(layout.config().join("settings.json"), b"{}").unwrap();
    layout.ensure().unwrap();

    for folder in layout.folders() {
        assert!(folder.is_dir(), "{}", folder.display());
    }
    assert!(layout.config().join("settings.json").is_file());
    let left: Vec<_> = std::fs::read_dir(layout.root())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert!(
        !left
            .iter()
            .any(|n| n.to_string_lossy().starts_with(".write-test"))
    );
}

#[test]
fn an_unwritable_root_is_an_error_naming_the_path() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("not-a-folder");
    std::fs::write(&blocker, b"").unwrap();
    let layout = Layout::in_folder(&blocker);

    let err = layout.ensure().unwrap_err();
    assert!(matches!(&err, Error::NotWritable { path, .. } if path.starts_with(&blocker)));
    assert!(err.to_string().contains("not writable"));
}

#[test]
fn windows_redirects_only_the_temp_folder() {
    let layout = Layout::in_folder(Path::new(r"D:\Мои программы"));
    let plan = redirect(
        &layout,
        Platform::Windows,
        &env(&[("APPDATA", r"C:\Users\a\AppData\Roaming")]),
    );
    assert_eq!(plan.set.len(), 2);
    assert_eq!(
        set_value(&plan, "TEMP"),
        Some(layout.tmp().into_os_string())
    );
    assert_eq!(set_value(&plan, "TMP"), Some(layout.tmp().into_os_string()));
    assert!(plan.restore.is_empty());
}

#[test]
fn linux_redirects_the_xdg_folders_and_remembers_the_originals() {
    let layout = Layout::in_folder(Path::new("/media/usb/cogit"));
    let get = env(&[
        ("HOME", "/home/ann"),
        ("XDG_CONFIG_HOME", "/home/ann/.cfg"),
        ("XDG_DATA_DIRS", "/tmp/.mount_x/usr/share:/usr/share"),
    ]);
    let plan = redirect(&layout, Platform::Linux, &get);

    assert_eq!(
        set_value(&plan, "XDG_CONFIG_HOME"),
        Some(layout.config().into_os_string())
    );
    assert_eq!(
        set_value(&plan, "XDG_DATA_HOME"),
        Some(layout.local().into_os_string())
    );
    assert_eq!(
        set_value(&plan, "XDG_CACHE_HOME"),
        Some(layout.cache().into_os_string())
    );
    assert_eq!(
        set_value(&plan, "TMPDIR"),
        Some(layout.tmp().into_os_string())
    );
    assert_eq!(
        set_value(&plan, "XDG_CONFIG_DIRS"),
        Some(OsString::from("/home/ann/.cfg:/etc/xdg"))
    );
    assert_eq!(
        set_value(&plan, "XDG_DATA_DIRS"),
        Some(OsString::from(
            "/home/ann/.local/share:/tmp/.mount_x/usr/share:/usr/share"
        ))
    );
    let restored: HashMap<_, _> = plan.restore.iter().cloned().collect();
    assert_eq!(
        restored["XDG_CONFIG_HOME"],
        Some(OsString::from("/home/ann/.cfg"))
    );
    assert_eq!(restored["XDG_DATA_HOME"], None);
    assert!(!restored.contains_key("TMPDIR"));
}

#[test]
fn linux_without_home_does_not_invent_search_paths() {
    let layout = Layout::in_folder(Path::new("/x"));
    let plan = redirect(&layout, Platform::Linux, &env(&[]));
    assert_eq!(
        set_value(&plan, "XDG_CONFIG_DIRS"),
        Some(OsString::from("/etc/xdg"))
    );
}

#[test]
fn a_child_gets_the_original_values_back_and_loses_the_ones_that_were_unset() {
    let layout = Layout::in_folder(Path::new("/x"));
    let plan = redirect(
        &layout,
        Platform::Linux,
        &env(&[
            ("HOME", "/home/ann"),
            ("XDG_CONFIG_HOME", "/home/ann/.config"),
        ]),
    );
    let mut command = std::process::Command::new("git");
    apply_restore(&mut command, &plan.restore);
    let envs: HashMap<_, _> = command
        .get_envs()
        .map(|(k, v)| (k.to_string_lossy().into_owned(), v.map(OsStr::to_owned)))
        .collect();

    assert_eq!(
        envs["XDG_CONFIG_HOME"],
        Some(OsString::from("/home/ann/.config"))
    );
    assert_eq!(envs["XDG_DATA_HOME"], None);
    assert_eq!(envs["XDG_STATE_HOME"], None);
    assert!(!envs.contains_key("TMPDIR"));
}

#[test]
fn nothing_is_created_or_named_outside_the_data_folder() {
    let outer = tempfile::tempdir().unwrap();
    let binary = outer.path().join("Cogit портативный");
    std::fs::create_dir_all(&binary).unwrap();
    std::fs::write(binary.join("cogit.exe"), b"").unwrap();
    let layout = Layout::in_folder(&binary_folder(&binary.join("cogit.exe"), None).unwrap());
    layout.ensure().unwrap();

    let mut named: Vec<PathBuf> = layout.folders().to_vec();
    for platform in [Platform::Windows, Platform::Linux, Platform::Other] {
        let plan = redirect(&layout, platform, &env(&[("HOME", "/home/ann")]));
        for (name, value) in &plan.set {
            if name.contains("DIRS") {
                continue;
            }
            named.push(PathBuf::from(value));
        }
    }
    for path in &named {
        assert!(path.starts_with(layout.root()), "{}", path.display());
    }

    let beside: Vec<_> = std::fs::read_dir(&binary)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(beside.len(), 2, "{beside:?}");
    assert!(beside.contains(&DATA_DIR.to_owned()) && beside.contains(&"cogit.exe".to_owned()));
    assert_eq!(std::fs::read_dir(outer.path()).unwrap().count(), 1);
}

#[test]
fn the_memory_settings_backend_is_for_a_session_without_a_bus_only() {
    // WSLg and bare containers: no bus, so dconf cannot be reached and warns on every write.
    assert!(needs_memory_gsettings(None, None, false));
    // A real desktop session: the bus exists, in the environment or as the runtime socket.
    assert!(!needs_memory_gsettings(
        None,
        Some(OsStr::new("unix:path=/run/user/1000/bus")),
        false
    ));
    assert!(!needs_memory_gsettings(None, None, true));
    // The user chose a backend: it stays.
    assert!(!needs_memory_gsettings(
        Some(OsStr::new("keyfile")),
        None,
        false
    ));
}
