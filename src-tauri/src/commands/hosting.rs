use super::blocking;
use git_engine::GitError;

fn hosting_error(err: app_state::HostingError) -> GitError {
    GitError::InvalidState(err.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn hosting_accounts(
    state: tauri::State<'_, crate::AppContext>,
) -> Result<app_state::HostingAccounts, GitError> {
    let app_state = state.state.clone();
    blocking("hosting_accounts", move || {
        app_state.hosting_accounts().map_err(hosting_error)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn add_hosting_account(
    state: tauri::State<'_, crate::AppContext>,
    account: app_state::NewHostingAccount,
) -> Result<app_state::HostingAccount, GitError> {
    let app_state = state.state.clone();
    blocking("add_hosting_account", move || {
        app_state
            .add_hosting_account(&account)
            .map_err(hosting_error)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn check_hosting_account(
    state: tauri::State<'_, crate::AppContext>,
    id: String,
) -> Result<String, GitError> {
    let app_state = state.state.clone();
    blocking("check_hosting_account", move || {
        app_state.check_hosting_account(&id).map_err(hosting_error)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_hosting_account(
    state: tauri::State<'_, crate::AppContext>,
    id: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("remove_hosting_account", move || {
        app_state.remove_hosting_account(&id).map_err(hosting_error)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn hosting_account_for(
    state: tauri::State<'_, crate::AppContext>,
    url: String,
) -> Result<Option<app_state::HostingMatch>, GitError> {
    let app_state = state.state.clone();
    blocking("hosting_account_for", move || {
        app_state.hosting_account_for(&url).map_err(hosting_error)
    })
    .await
}
