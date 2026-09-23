//! The desktop's own actions on a folder or a file (#36, #40): which ones this platform
//! has, and starting them. The programs and their arguments are chosen in
//! `app_state::desktop`; these only start them.

use app_state::RepoId;
use app_state::desktop::{self, DesktopInfo, Launch, Platform};
use git_engine::GitError;

use super::blocking;

fn start(label: &'static str, launch: Launch, cwd: Option<String>) -> Result<(), GitError> {
    tracing::info!(program = %launch.program, args = ?launch.args, label, "starting a desktop program");
    desktop::spawn(&launch, cwd.as_deref().map(std::path::Path::new))
        .map_err(|err| GitError::Io(format!("cannot start {}: {err}", launch.program)))
}

/// Looks for Git Bash on disk and in the registry, so it stays off the main thread.
#[tauri::command]
#[specta::specta]
pub async fn desktop_info() -> Result<DesktopInfo, GitError> {
    blocking("desktop_info", || Ok(desktop::info())).await
}

/// A folder opens itself; a file opens in the application paired with it.
#[tauri::command]
#[specta::specta]
pub async fn open_path(path: String) -> Result<(), GitError> {
    blocking("open_path", move || {
        start(
            "open",
            desktop::open_command(Platform::current(), &path),
            None,
        )
    })
    .await
}

/// The parent folder with the item selected.
#[tauri::command]
#[specta::specta]
pub async fn reveal_path(path: String) -> Result<(), GitError> {
    blocking("reveal_path", move || {
        start(
            "reveal",
            desktop::reveal_command(Platform::current(), &path),
            None,
        )
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn open_power_shell(path: String) -> Result<(), GitError> {
    blocking("open_power_shell", move || {
        let launch = desktop::power_shell_command(Platform::current())
            .ok_or_else(|| GitError::InvalidState("PowerShell is a Windows program".to_owned()))?;
        start("powershell", launch, Some(path))
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn open_git_shell(path: String) -> Result<(), GitError> {
    blocking("open_git_shell", move || {
        let bash = desktop::find_git_bash().ok_or_else(|| {
            GitError::InvalidState("Git Bash from Git for Windows was not found".to_owned())
        })?;
        let launch = desktop::git_shell_command(&bash, Platform::current(), &path);
        start("git-shell", launch, Some(path))
    })
    .await
}

/// Repository-relative paths, moved to the Recycle Bin (the bin of the desktop elsewhere).
#[tauri::command]
#[specta::specta]
pub async fn move_to_trash(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    paths: Vec<String>,
) -> Result<(), GitError> {
    let root = state.state.root_of(repo)?;
    blocking("move_to_trash", move || {
        let absolute: Vec<std::path::PathBuf> = paths
            .iter()
            .map(|path| root.join(path.trim_end_matches('/')))
            .collect();
        crate::recycle_bin::move_to_trash(&absolute).map_err(|err| GitError::Io(err.to_string()))
    })
    .await
}
