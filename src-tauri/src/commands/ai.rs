use super::blocking;
use app_state::{AiConfig, AiKeyInput, AiKeyState, AiMessage, AiMessageRequest, RepoId};
use git_engine::GitError;
use serde::Serialize;

#[tauri::command]
#[specta::specta]
pub async fn ai_key_state(
    state: tauri::State<'_, crate::AppContext>,
    config: AiConfig,
) -> Result<AiKeyState, GitError> {
    let app_state = state.state.clone();
    blocking("ai_key_state", move || {
        app_state.ai_key_state(&config).map_err(GitError::from)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn set_ai_key(
    state: tauri::State<'_, crate::AppContext>,
    input: AiKeyInput,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("set_ai_key", move || {
        app_state.set_ai_key(&input).map_err(GitError::from)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_ai_key(
    state: tauri::State<'_, crate::AppContext>,
    config: AiConfig,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("remove_ai_key", move || {
        app_state.remove_ai_key(&config).map_err(GitError::from)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn test_ai_provider(
    state: tauri::State<'_, crate::AppContext>,
    config: AiConfig,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("test_ai_provider", move || {
        app_state.test_ai_provider(&config).map_err(GitError::from)
    })
    .await
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum AiChunk {
    Started { id: u32 },
    Text { text: String },
}

#[tauri::command]
#[specta::specta]
pub async fn generate_commit_message(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    request: AiMessageRequest,
    on_chunk: tauri::ipc::Channel<AiChunk>,
) -> Result<AiMessage, GitError> {
    let app_state = state.state.clone();
    let run = state.state.start_read();
    let cancel = run.token();
    let _ = on_chunk.send(AiChunk::Started { id: run.id() });
    let result = blocking("generate_commit_message", move || {
        app_state
            .generate_commit_message(repo, &request, &|| cancel.is_cancelled(), &mut |text| {
                let _ = on_chunk.send(AiChunk::Text {
                    text: text.to_owned(),
                });
            })
            .map_err(GitError::from)
    })
    .await;
    drop(run);
    result
}
