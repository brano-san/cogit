//! `cogit <path>` (F-946): the request from this process's
//! command line, and from every later `cogit` the single-instance plugin hands over. The
//! page pulls them, so one made before it listens is not lost.

use desktop_entry::launch::{self, Show};
use parking_lot::Mutex;
use std::ffi::OsString;
use std::path::Path;
use tauri::Manager as _;
use tauri_specta::Event as _;

#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum LaunchShow {
    Repository,
}

/// `folder` is `path` itself for a folder (or a path that does not exist), else its parent:
/// what is handed to `open_repository`.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LaunchPath {
    pub path: String,
    pub folder: String,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRequest {
    pub show: LaunchShow,
    pub paths: Vec<LaunchPath>,
}

/// Another `cogit` handed its command line over; `take_launch_requests` has it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRequested {
    pub queued: u32,
}

#[derive(Default)]
pub struct Pending(Mutex<Vec<LaunchRequest>>);

impl Pending {
    pub fn new(first: Option<LaunchRequest>) -> Self {
        Self(Mutex::new(first.into_iter().collect()))
    }
}

/// `None` for a bare `cogit`; `main` refused a malformed command line before the window.
pub fn request<I: IntoIterator<Item = OsString>>(args: I, cwd: &Path) -> Option<LaunchRequest> {
    match launch::parse(args) {
        Ok(command) => command.map(|command| to_dto(launch::resolve(command, cwd))),
        Err(message) => {
            tracing::warn!(%message, "a command line Cogit cannot follow");
            None
        }
    }
}

fn to_dto(request: launch::Request) -> LaunchRequest {
    let show = match request.show {
        Show::Repository => LaunchShow::Repository,
    };
    let paths = request
        .paths
        .into_iter()
        .map(|path| {
            let folder = match path.parent() {
                Some(parent) if path.is_file() => parent.to_path_buf(),
                _ => path.clone(),
            };
            LaunchPath {
                path: text(&path),
                folder: text(&folder),
            }
        })
        .collect();
    LaunchRequest { show, paths }
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// What the plugin passes: `args` starts with the other process's executable, and relative
/// paths are that process's, resolved against its working directory.
fn handed_over(args: Vec<String>, cwd: &str) -> Option<LaunchRequest> {
    request(args.into_iter().skip(1).map(OsString::from), Path::new(cwd))
}

/// The plugin's callback.
pub fn second_instance(app: &tauri::AppHandle, args: Vec<String>, cwd: String) {
    let request = handed_over(args, &cwd);
    tracing::info!(?request, %cwd, "another cogit handed its command line over");
    if let Some(window) = app.get_webview_window(crate::child_window::MAIN)
        && let Err(err) = window
            .unminimize()
            .and_then(|()| window.show())
            .and_then(|()| window.set_focus())
    {
        tracing::warn!(error = %err, "cannot bring the main window forward for another cogit");
    }
    let Some(request) = request else {
        return;
    };
    let pending = app.state::<Pending>();
    let queued = {
        let mut queue = pending.0.lock();
        queue.push(request);
        u32::try_from(queue.len()).unwrap_or(u32::MAX)
    };
    if let Err(err) = (LaunchRequested { queued }).emit(app) {
        tracing::error!(error = ?err, context = "launch request");
    }
}

/// Everything asked for since the last call, oldest first.
#[tauri::command]
#[specta::specta]
pub fn take_launch_requests(pending: tauri::State<'_, Pending>) -> Vec<LaunchRequest> {
    let taken = std::mem::take(&mut *pending.0.lock());
    if !taken.is_empty() {
        tracing::info!(requests = ?taken, "the window takes the command line");
    }
    taken
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn a_bare_cogit_hands_over_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy();
        assert!(handed_over(args(&["cogit.exe"]), &cwd).is_none());
    }

    #[test]
    fn the_other_process_folder_is_where_its_paths_start() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub").join("a.rs"), "").unwrap();
        let cwd = dir.path().to_string_lossy();

        let open = handed_over(args(&["cogit.exe", "sub", "missing"]), &cwd).unwrap();
        assert!(matches!(open.show, LaunchShow::Repository));
        let sub = text(&dir.path().join("sub"));
        let missing = text(&dir.path().join("missing"));
        assert_eq!(open.paths[0].path, sub);
        assert_eq!(open.paths[0].folder, sub);
        assert_eq!(open.paths[1].folder, missing);

        let file = handed_over(args(&["cogit.exe", "sub/a.rs"]), &cwd).unwrap();
        assert_eq!(
            file.paths[0].path,
            text(&dir.path().join("sub").join("a.rs"))
        );
        assert_eq!(file.paths[0].folder, sub);
    }

    #[test]
    fn requests_wait_until_the_page_takes_them() {
        let pending = Pending::new(None);
        assert!(pending.0.lock().is_empty());
        let first = LaunchRequest {
            show: LaunchShow::Repository,
            paths: Vec::new(),
        };
        let pending = Pending::new(Some(first));
        assert_eq!(std::mem::take(&mut *pending.0.lock()).len(), 1);
        assert!(pending.0.lock().is_empty());
    }
}
