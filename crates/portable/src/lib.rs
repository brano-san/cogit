use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const DATA_DIR: &str = "Cogit-data";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot tell which folder the Cogit binary is in: {0}")]
    NoBinaryFolder(String),
    #[error("the portable data folder {path} is not writable: {source}")]
    NotWritable {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// `<binary folder>/Cogit-data` and its subfolders; nothing the app writes lives elsewhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    root: PathBuf,
}

impl Layout {
    #[must_use]
    pub fn in_folder(binary_folder: &Path) -> Self {
        Self {
            root: binary_folder.join(DATA_DIR),
        }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// settings.json, presets, window state.
    #[must_use]
    pub fn config(&self) -> PathBuf {
        self.root.join("config")
    }

    /// The webview profile (cookies, localStorage), and on Linux the XDG data home.
    #[must_use]
    pub fn local(&self) -> PathBuf {
        self.root.join("local")
    }

    #[must_use]
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }

    #[must_use]
    pub fn tmp(&self) -> PathBuf {
        self.root.join("tmp")
    }

    #[must_use]
    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }

    #[must_use]
    pub fn folders(&self) -> [PathBuf; 5] {
        [
            self.config(),
            self.local(),
            self.logs(),
            self.tmp(),
            self.cache(),
        ]
    }

    /// Creates what is missing and proves the folder takes a file: a read-only medium
    /// must stop the start, never push the writes somewhere else.
    pub fn ensure(&self) -> Result<(), Error> {
        for folder in self.folders() {
            std::fs::create_dir_all(&folder).map_err(|source| Error::NotWritable {
                path: folder,
                source,
            })?;
        }
        let probe = self
            .root
            .join(format!(".write-test-{}", std::process::id()));
        std::fs::write(&probe, b"").map_err(|source| Error::NotWritable {
            path: self.root.clone(),
            source,
        })?;
        let _ = std::fs::remove_file(&probe);
        Ok(())
    }
}

/// Where the portable build lives. A running AppImage executes from a read-only squashfs
/// mount, so its own folder is the one holding the `.AppImage` file (`$APPIMAGE`).
#[must_use]
pub fn binary_folder(exe: &Path, appimage: Option<&Path>) -> Option<PathBuf> {
    let parent_of = |path: &Path| {
        path.parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
    };
    appimage
        .filter(|path| !path.as_os_str().is_empty())
        .and_then(parent_of)
        .or_else(|| parent_of(exe))
}

/// `\\?\C:\x` becomes `C:\x`: the prefix `canonicalize` adds on Windows is not welcome to
/// every consumer of the path (WebView2's profile folder among them). UNC and device forms
/// stay: without the prefix they name something else.
#[must_use]
pub fn strip_verbatim(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path.to_path_buf(),
    }
}

/// `canonicalize` without the verbatim prefix; a path it cannot resolve stays as given.
#[must_use]
pub fn real_path(path: &Path) -> PathBuf {
    strip_verbatim(&path.canonicalize().unwrap_or_else(|_| path.to_path_buf()))
}

/// The real folder of this process's binary, symlinks followed.
pub fn locate() -> Result<PathBuf, Error> {
    let exe = std::env::current_exe().map_err(|err| Error::NoBinaryFolder(err.to_string()))?;
    let appimage = std::env::var_os("APPIMAGE").map(|path| real_path(Path::new(&path)));
    binary_folder(&real_path(&exe), appimage.as_deref())
        .ok_or_else(|| Error::NoBinaryFolder("the executable has no parent folder".into()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    Linux,
    Other,
}

impl Platform {
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }
}

pub type Restore = Vec<(&'static str, Option<OsString>)>;

/// What the process sets for itself, and what every child gets back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redirect {
    pub set: Vec<(&'static str, OsString)>,
    pub restore: Restore,
}

const SYSTEM_CONFIG_DIRS: &str = "/etc/xdg";
const SYSTEM_DATA_DIRS: &str = "/usr/local/share:/usr/share";

/// Windows: `dirs` and Tauri ask the shell for known folders and ignore `APPDATA`, so only
/// the temp folder follows the environment there; the writers get explicit paths.
/// Linux: GLib, WebKitGTK and `dirs` follow the XDG variables. The user's own folders are
/// appended to the search paths, so themes, MIME associations and `.desktop` files still
/// resolve for reading. Temp folders stay redirected for children too (git's scratch files
/// are Cogit's writes in the user's eyes); the XDG variables go back to what they were,
/// so git, ssh and credential helpers read the user's real configuration.
#[must_use]
pub fn redirect(
    layout: &Layout,
    platform: Platform,
    get: &dyn Fn(&str) -> Option<OsString>,
) -> Redirect {
    let tmp = layout.tmp().into_os_string();
    match platform {
        Platform::Windows => Redirect {
            set: vec![("TEMP", tmp.clone()), ("TMP", tmp)],
            restore: Vec::new(),
        },
        Platform::Other => Redirect {
            set: vec![("TMPDIR", tmp)],
            restore: Vec::new(),
        },
        Platform::Linux => linux(layout, tmp, get),
    }
}

fn is_unix_absolute(value: &OsStr) -> bool {
    value.as_encoded_bytes().first() == Some(&b'/')
}

fn linux(layout: &Layout, tmp: OsString, get: &dyn Fn(&str) -> Option<OsString>) -> Redirect {
    let user_home = |name: &str, fallback: &str| {
        get(name)
            .filter(|value| is_unix_absolute(value))
            .or_else(|| {
                get("HOME")
                    .filter(|home| is_unix_absolute(home))
                    .map(|mut path| {
                        path.push("/");
                        path.push(fallback);
                        path
                    })
            })
    };
    let search = |own: Option<OsString>, name: &str, system: &str| {
        let mut paths = own.unwrap_or_default();
        if !paths.is_empty() {
            paths.push(":");
        }
        paths.push(
            get(name)
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| system.into()),
        );
        paths
    };
    let config_dirs = search(
        user_home("XDG_CONFIG_HOME", ".config"),
        "XDG_CONFIG_DIRS",
        SYSTEM_CONFIG_DIRS,
    );
    let data_dirs = search(
        user_home("XDG_DATA_HOME", ".local/share"),
        "XDG_DATA_DIRS",
        SYSTEM_DATA_DIRS,
    );
    let restore = [
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_CACHE_HOME",
        "XDG_STATE_HOME",
        "XDG_CONFIG_DIRS",
        "XDG_DATA_DIRS",
    ]
    .map(|name| (name, get(name)))
    .to_vec();
    Redirect {
        set: vec![
            ("XDG_CONFIG_HOME", layout.config().into_os_string()),
            ("XDG_DATA_HOME", layout.local().into_os_string()),
            ("XDG_CACHE_HOME", layout.cache().into_os_string()),
            (
                "XDG_STATE_HOME",
                layout.local().join("state").into_os_string(),
            ),
            ("XDG_CONFIG_DIRS", config_dirs),
            ("XDG_DATA_DIRS", data_dirs),
            ("TMPDIR", tmp),
        ],
        restore,
    }
}

/// Gives a child the original value of each redirected variable, or none where there was
/// none.
pub fn apply_restore(command: &mut std::process::Command, restore: &Restore) {
    for (name, value) in restore {
        match value {
            Some(value) => command.env(name, value),
            None => command.env_remove(name),
        };
    }
}

struct Active {
    layout: Layout,
    restore: Restore,
}

static ACTIVE: OnceLock<Active> = OnceLock::new();

/// Creates the folders and points the process at them. Before any other thread exists:
/// `set_var` is not thread-safe, and GTK, Tauri and the logger read the variables once.
pub fn activate(layout: Layout) -> Result<&'static Layout, Error> {
    if let Some(active) = ACTIVE.get() {
        return Ok(&active.layout);
    }
    layout.ensure()?;
    let plan = redirect(&layout, Platform::current(), &|name: &str| {
        std::env::var_os(name)
    });
    for (name, value) in &plan.set {
        set_var(name, value);
    }
    let active = ACTIVE.get_or_init(|| Active {
        layout,
        restore: plan.restore,
    });
    Ok(&active.layout)
}

#[allow(unsafe_code)]
fn set_var(name: &str, value: &OsStr) {
    // SAFETY: called from `activate`, at the start of `run()`, while the process is
    // single-threaded.
    unsafe { std::env::set_var(name, value) }
}

/// GSettings (GTK's file chooser, themes) writes through dconf. Without a session bus (WSLg,
/// a bare container) every write logs `dconf-WARNING: failed to commit changes`: the in-memory
/// backend then keeps the settings for the run and the console quiet. A desktop with a bus,
/// or a backend the user chose, is left alone.
#[must_use]
pub fn needs_memory_gsettings(
    backend: Option<&OsStr>,
    bus_address: Option<&OsStr>,
    bus_socket_exists: bool,
) -> bool {
    backend.is_none() && bus_address.is_none() && !bus_socket_exists
}

/// First thing in `run()`, like `activate`: single-threaded, before GTK reads the environment.
pub fn quiet_gsettings_without_a_bus() {
    let bus_socket_exists =
        std::env::var_os("XDG_RUNTIME_DIR").is_some_and(|dir| Path::new(&dir).join("bus").exists());
    if needs_memory_gsettings(
        std::env::var_os("GSETTINGS_BACKEND").as_deref(),
        std::env::var_os("DBUS_SESSION_BUS_ADDRESS").as_deref(),
        bus_socket_exists,
    ) {
        set_var("GSETTINGS_BACKEND", OsStr::new("memory"));
    }
}

#[must_use]
pub fn layout() -> Option<&'static Layout> {
    ACTIVE.get().map(|active| &active.layout)
}

/// For every process Cogit starts on the user's behalf; a no-op in a normal build.
pub fn restore_child_env(command: &mut std::process::Command) {
    if let Some(active) = ACTIVE.get() {
        apply_restore(command, &active.restore);
    }
}

#[cfg(test)]
mod tests;
