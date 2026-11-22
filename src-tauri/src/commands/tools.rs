use app_state::user_commands::{ToolDefinition, ToolFile, ToolPreview, ToolRequest, ToolRun};
use app_state::{OperationKind, RepoId};
use git_engine::GitError;
use std::path::PathBuf;

/// Captured: a cancellable queue operation, like a hook's dry run. Detached: no queue turn.
#[tauri::command]
#[specta::specta]
pub async fn run_user_command(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    request: ToolRequest,
) -> Result<ToolRun, GitError> {
    let app_state = state.state.clone();
    let config_dir = state.config_dir.clone();
    if request.detached {
        return super::blocking("run_user_command", move || {
            app_state.run_user_command(
                repo,
                &request,
                &git_engine::NetworkStop::default(),
                &config_dir,
            )
        })
        .await;
    }
    let title = format!("Running {}", request.name);
    super::network::networking_if(
        true,
        &state.state,
        repo,
        OperationKind::Other,
        &title,
        "run_user_command",
        move |stop| app_state.run_user_command(repo, &request, &stop, &config_dir),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn preview_user_command(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    request: ToolRequest,
) -> Result<ToolPreview, GitError> {
    let (app, config) = (state.state.clone(), state.config_dir.clone());
    super::blocking("preview_user_command", move || {
        app.preview_user_command(repo, &request, &config)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn repository_tools(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Option<ToolFile>, GitError> {
    let (app, config) = (state.state.clone(), state.config_dir.clone());
    super::blocking("repository_tools", move || {
        app.repository_tools(repo, &config)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn trust_repository_tools(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    hash: String,
) -> Result<(), GitError> {
    let (app, config) = (state.state.clone(), state.config_dir.clone());
    super::blocking("trust_repository_tools", move || {
        app.trust_repository_tools(repo, &config, &hash)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn import_tool_commands(path: PathBuf) -> Result<Vec<ToolDefinition>, GitError> {
    super::blocking("import_tool_commands", move || {
        app_state::user_commands::import_commands(&path)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn export_tool_commands(
    path: PathBuf,
    commands: Vec<ToolDefinition>,
) -> Result<(), GitError> {
    super::blocking("export_tool_commands", move || {
        app_state::user_commands::export_commands(&path, &commands)
    })
    .await
}
