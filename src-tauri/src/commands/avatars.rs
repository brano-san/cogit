//! Author pictures: the visible window, the rows, on and off (M14).

use super::blocking;
use git_engine::GitError;
use tauri::Manager as _;

/// The addresses on screen, for the download queue. Sent on every scroll, so it reads
/// nothing: rows that scrolled away leave the queue, the rest keep their place in it.
#[tauri::command]
#[specta::specta]
pub async fn avatar_window(
    state: tauri::State<'_, crate::AppContext>,
    emails: Vec<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("avatar_window", move || {
        app_state.avatar_window(&emails);
        Ok(())
    })
    .await
}

/// The pictures these authors already have. One file read and one base64 encode each,
/// so the caller asks only for what it does not hold (M14 T14.2).
#[tauri::command]
#[specta::specta]
pub async fn avatars(
    state: tauri::State<'_, crate::AppContext>,
    authors: Vec<app_state::Author>,
) -> Result<Vec<app_state::AvatarRow>, GitError> {
    let state = state.state.clone();
    blocking("avatars", move || Ok(state.avatars(&authors))).await
}

/// Turning avatars on is also what creates the cache directory: off means no directory,
/// no request and no address leaving the machine (M14 T14.3).
#[tauri::command]
#[specta::specta]
pub async fn set_avatars(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppContext>,
    enabled: bool,
) -> Result<(), GitError> {
    let state = state.state.clone();
    if !enabled {
        return blocking("set_avatars", move || {
            state.disable_avatars();
            Ok(())
        })
        .await;
    }

    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|err| GitError::Io(format!("no cache directory: {err}")))?
        .join("avatars");

    blocking("set_avatars", move || {
        state
            .enable_avatars(dir)
            .map_err(|err| GitError::Io(err.to_string()))
    })
    .await
}
