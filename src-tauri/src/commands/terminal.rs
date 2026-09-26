//! Opening a terminal at a repository; `app_state::terminal` chooses the program.

use super::blocking;
use git_engine::GitError;

/// The terminals this platform can offer, for the settings dropdown.
#[tauri::command]
#[specta::specta]
pub fn terminal_choices() -> Vec<TerminalChoice> {
    app_state::terminal::choices()
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
    let kind = app_state::terminal::Terminal::from_id(&terminal).unwrap_or_default();
    let launch = app_state::terminal::launch_for(kind, &path);

    blocking("open_in_terminal", move || {
        app_state::desktop::spawn(&launch, Some(std::path::Path::new(&path)))
            .map_err(|err| GitError::Io(format!("cannot start {}: {err}", launch.program)))
    })
    .await
}
