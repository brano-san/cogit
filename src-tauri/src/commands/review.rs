use super::blocking;
use app_state::{LineCounts, RepoId, ReviewPath};
use git_engine::{BranchReview, DiffSpec, FileEntry, GitError};

const CHUNK: usize = 200;

#[tauri::command]
#[specta::specta]
pub async fn review_base(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    branch: String,
) -> Result<Option<String>, GitError> {
    let app_state = state.state.clone();
    blocking("review_base", move || app_state.review_base(repo, &branch)).await
}

#[tauri::command]
#[specta::specta]
pub async fn branch_review(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    base: String,
    branch: String,
) -> Result<BranchReview, GitError> {
    let app_state = state.state.clone();
    blocking("branch_review", move || {
        app_state.branch_review(repo, &base, &branch)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn review_files(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    merge_base: String,
    branch: String,
    worktree: bool,
    on_chunk: tauri::ipc::Channel<Vec<FileEntry>>,
) -> Result<u32, GitError> {
    let app_state = state.state.clone();
    let files = blocking("review_files", move || {
        app_state.review_files(repo, &merge_base, &branch, worktree)
    })
    .await?;
    let total = u32::try_from(files.len()).unwrap_or(u32::MAX);
    for chunk in super::in_chunks(files, CHUNK) {
        let _ = on_chunk.send(chunk);
    }
    Ok(total)
}

#[tauri::command]
#[specta::specta]
pub async fn review_line_counts(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    spec: DiffSpec,
    files: Vec<ReviewPath>,
) -> Result<Vec<LineCounts>, GitError> {
    let app_state = state.state.clone();
    blocking("review_line_counts", move || {
        app_state.review_line_counts(repo, &spec, &files)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn open_review_window(
    app: tauri::AppHandle,
    url: String,
    title: String,
) -> Result<(), GitError> {
    blocking("open_review_window", move || {
        crate::child_window::open(
            &app,
            "review",
            url,
            title,
            crate::child_window::Shape {
                width: 1280.0,
                height: 860.0,
                min_width: 820.0,
                min_height: 520.0,
            },
        )
        .map(drop)
        .map_err(|err| GitError::Internal(format!("cannot open the Review Branch window: {err}")))
    })
    .await
}
