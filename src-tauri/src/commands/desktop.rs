//! Desktop actions (#36, #40); `app_state::desktop` chooses the program and arguments.

use app_state::RepoId;
use app_state::desktop::{self, DesktopInfo, Launch, Platform};
use git_engine::GitError;

use super::{blocking, mutating};

pub(super) fn start(
    label: &'static str,
    launch: Launch,
    cwd: Option<String>,
) -> Result<(), GitError> {
    tracing::info!(program = %launch.program, args = ?launch.args, label, "starting a desktop program");
    desktop::spawn(&launch, cwd.as_deref().map(std::path::Path::new))
        .map_err(|err| GitError::Io(format!("cannot start {}: {err}", launch.program)))
}

/// Looks for Git Bash on disk and in the registry, so it stays off the main thread.
#[tauri::command]
#[specta::specta]
pub async fn desktop_info() -> Result<DesktopInfo, GitError> {
    blocking("desktop_info", || Ok(desktop::info(Platform::current()))).await
}

/// The frontend's theme is dark or light; GTK dialogs follow it (Linux), a no-op elsewhere.
#[tauri::command]
#[specta::specta]
pub async fn set_native_theme(app: tauri::AppHandle, dark: bool) {
    crate::native_theme::apply(&app, dark);
}

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

/// A link in the browser. Not the opener plugin's `openUrl` from the page: it starts the
/// browser with a portable build's redirected `XDG_*` (a second, empty profile).
#[tauri::command]
#[specta::specta]
pub async fn open_url(app: tauri::AppHandle, url: String) -> Result<(), GitError> {
    match desktop::url_command(Platform::current(), &url).map_err(GitError::InvalidState)? {
        Some(launch) => blocking("open_url", move || start("url", launch, None)).await,
        None => tauri_plugin_opener::OpenerExt::opener(&app)
            .open_url(&url, None::<&str>)
            .map_err(|err| GitError::Io(format!("cannot open {url}: {err}"))),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn reveal_path(path: String) -> Result<(), GitError> {
    blocking("reveal_path", move || {
        tracing::info!(path = %path, "revealing in the file manager");
        desktop::reveal(Platform::current(), &path)
            .map_err(|err| GitError::Io(format!("cannot show {path} in the file manager: {err}")))
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

#[tauri::command]
#[specta::specta]
pub async fn move_to_trash(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    paths: Vec<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    mutating(
        &state.state,
        repo,
        app_state::OperationKind::Discard,
        "move_to_trash",
        move || app_state.move_to_trash(repo, &paths, crate::recycle_bin::move_to_trash),
    )
    .await
}
