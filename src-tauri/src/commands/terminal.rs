//! Opening a terminal at a repository; `app_state::terminal` chooses the program.

use super::blocking;
use app_state::desktop::{self, Platform};
use app_state::terminal::{Terminal, choices, launch_for};
use git_engine::GitError;

/// The terminals this platform can offer, for the settings dropdown.
#[tauri::command]
#[specta::specta]
pub fn terminal_choices() -> Vec<TerminalChoice> {
    choices(Platform::current())
        .into_iter()
        .map(|kind| TerminalChoice {
            id: kind.id().to_owned(),
            label: kind.label().to_owned(),
        })
        .collect()
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TerminalChoice {
    pub id: String,
    pub label: String,
}

/// Spawned with the repository as its working directory, detached from Cogit: closing the
/// client must not close the user's shell.
#[tauri::command]
#[specta::specta]
pub async fn open_in_terminal(path: String, terminal: String) -> Result<(), GitError> {
    blocking("open_in_terminal", move || {
        let platform = Platform::current();
        let kind = Terminal::from_id(platform, &terminal).unwrap_or_default();
        // Registry and disk: looked up only for the one choice that needs it.
        let git_bash = (kind == Terminal::GitBash)
            .then(desktop::find_git_bash)
            .flatten();
        let launch = launch_for(platform, kind, &path, git_bash.as_deref());
        desktop::spawn(&launch, Some(std::path::Path::new(&path)))
            .map_err(|err| GitError::Io(format!("cannot start {}: {err}", launch.program)))
    })
    .await
}
