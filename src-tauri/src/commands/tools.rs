use app_state::user_commands::{ToolRequest, ToolRun};
use app_state::{OperationKind, RepoId};
use git_engine::GitError;

/// Captured: a cancellable queue operation, like a hook's dry run. Detached: no queue turn.
#[tauri::command]
#[specta::specta]
pub async fn run_user_command(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    request: ToolRequest,
) -> Result<ToolRun, GitError> {
    let app_state = state.state.clone();
    if request.detached {
        return super::blocking("run_user_command", move || {
            app_state.run_user_command(repo, &request, &git_engine::NetworkStop::default())
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
        move |stop| app_state.run_user_command(repo, &request, &stop),
    )
    .await
}
