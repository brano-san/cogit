use super::blocking;
use app_state::{LineCounts, RepoId, ReviewPath};
use git_engine::{BranchReview, DiffSpec, FileEntry, GitError, ReviewCommit};

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
    on_chunk: tauri::ipc::Channel<Vec<ReviewCommit>>,
) -> Result<BranchReview, GitError> {
    let app_state = state.state.clone();
    blocking("branch_review", move || {
        let mut review = app_state.branch_review(repo, &base, &branch)?;
        send_chunks(std::mem::take(&mut review.commits), |chunk| {
            on_chunk.send(chunk)
        })?;
        Ok(review)
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
    blocking("review_files", move || {
        let files = app_state.review_files(repo, &merge_base, &branch, worktree)?;
        send_chunks(files, |chunk| on_chunk.send(chunk))
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn review_commit_files(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    oid: String,
    on_chunk: tauri::ipc::Channel<Vec<FileEntry>>,
) -> Result<u32, GitError> {
    let app_state = state.state.clone();
    blocking("review_commit_files", move || {
        let files = app_state.commit_files(repo, &oid)?;
        send_chunks(files, |chunk| on_chunk.send(chunk))
    })
    .await
}

fn send_chunks<T>(
    items: Vec<T>,
    mut send: impl FnMut(Vec<T>) -> tauri::Result<()>,
) -> Result<u32, GitError> {
    let total = u32::try_from(items.len()).unwrap_or(u32::MAX);
    for chunk in super::in_chunks(items, CHUNK) {
        send(chunk)
            .map_err(|err| GitError::Internal(format!("cannot stream the review: {err}")))?;
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

#[cfg(test)]
mod tests {
    use super::send_chunks;

    #[test]
    fn streams_a_large_review_in_order() -> Result<(), git_engine::GitError> {
        let mut chunks = Vec::new();
        let total = send_chunks((0..1050).collect(), |chunk| {
            chunks.push(chunk);
            Ok(())
        })?;
        assert_eq!(total, 1050);
        assert_eq!(
            chunks.iter().map(Vec::len).collect::<Vec<_>>(),
            [200, 200, 200, 200, 200, 50]
        );
        assert!(chunks.into_iter().flatten().eq(0..1050));
        Ok(())
    }
}
