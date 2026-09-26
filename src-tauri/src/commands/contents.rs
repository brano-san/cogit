//! What a commit or the working tree holds: details, file lists, diffs, images, search.

use super::blocking;
use app_state::RepoId;
use diff_engine::{DiffOptions, FileDiff};
use git_engine::{CommitDetails, DiffSpec, FileEntry, GitError};
use serde::Serialize;

#[tauri::command]
#[specta::specta]
pub async fn commit_details(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
) -> Result<CommitDetails, GitError> {
    let app_state = state.state.clone();
    blocking("commit_details", move || {
        app_state.commit_details(repo, &rev)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn commit_files(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
) -> Result<Vec<FileEntry>, GitError> {
    let app_state = state.state.clone();
    let started = std::time::Instant::now();

    let files = blocking("commit_files", move || app_state.commit_files(repo, &rev)).await?;

    tracing::debug!(
        repo = repo.0,
        files = files.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "commit files listed"
    );
    Ok(files)
}

/// Every file of a commit's tree: the Files panel's Unchanged switch on a commit.
#[tauri::command]
#[specta::specta]
pub async fn commit_tree_files(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    rev: String,
) -> Result<Vec<String>, GitError> {
    let app_state = state.state.clone();
    blocking("commit_tree_files", move || {
        app_state.tree_files(repo, &rev)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn diff_file(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    spec: DiffSpec,
    path: String,
    options: DiffOptions,
) -> Result<FileDiff, GitError> {
    let app_state = state.state.clone();
    let started = std::time::Instant::now();
    let logged = path.clone();

    let diff = blocking("diff_file", move || {
        app_state.diff_file(repo, &spec, &path, &options)
    })
    .await?;

    tracing::debug!(
        repo = repo.0,
        path = %logged,
        elapsed_ms = started.elapsed().as_millis(),
        "file diff computed"
    );
    Ok(diff)
}

/// Every file of a commit in one round trip, diffed in parallel (doc/08-diff-engine.md §9).
#[tauri::command]
#[specta::specta]
pub async fn diff_files(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    spec: DiffSpec,
    paths: Vec<String>,
    options: DiffOptions,
    request: u32,
) -> Result<app_state::DiffBatch, GitError> {
    let app_state = state.state.clone();
    let started = std::time::Instant::now();
    let requested = paths.len();

    let batch = blocking("diff_files", move || {
        app_state.diff_files(repo, &spec, &paths, &options, request)
    })
    .await?;

    tracing::debug!(
        repo = repo.0,
        files = requested,
        request,
        superseded = matches!(batch, app_state::DiffBatch::Superseded),
        elapsed_ms = started.elapsed().as_millis(),
        "commit files diffed"
    );
    Ok(batch)
}

#[tauri::command]
#[specta::specta]
pub async fn image_sides(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    spec: DiffSpec,
    path: String,
) -> Result<(Option<String>, Option<String>), GitError> {
    let app_state = state.state.clone();
    blocking("image_sides", move || {
        app_state.image_sides(repo, &spec, &path)
    })
    .await
}

/// The file as it was before a commit. `None` means there was no such file to open.
#[tauri::command]
#[specta::specta]
pub async fn file_before(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    oid: String,
    path: String,
) -> Result<Option<String>, GitError> {
    let app_state = state.state.clone();
    blocking("file_before", move || {
        app_state.file_before(repo, &oid, &path)
    })
    .await
}

/// What travels up the channel while a content search runs.
///
/// `Started` comes first and carries the id, so the panel can cancel a search long before
/// it has an answer — which is the point when the user is typing.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SearchChunk {
    Started {
        id: u32,
    },
    Matches {
        matches: Vec<git_engine::ContentMatch>,
    },
    Done {
        total: u32,
        cancelled: bool,
    },
}

/// Searches inside files, streaming matches as they are found.
///
/// Cancellable: the id arrives on the first chunk and `cancel_operation` stops it.
/// Binary files and anything over two megabytes are skipped without being opened.
#[tauri::command]
#[specta::specta]
pub async fn search_file_contents(
    state: tauri::State<'_, crate::AppContext>,
    cancellations: tauri::State<'_, std::sync::Arc<crate::operations::Cancellations>>,
    repo: RepoId,
    query: String,
    is_regex: bool,
    scope: git_engine::SearchScope,
    on_chunk: tauri::ipc::Channel<SearchChunk>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    let cancellations = std::sync::Arc::clone(&cancellations);

    let (id, cancel) = cancellations.start();
    let _ = on_chunk.send(SearchChunk::Started { id });

    let token = cancel.clone();
    let channel = on_chunk.clone();
    let counted = blocking("search_file_contents", move || {
        let request = git_engine::SearchRequest {
            query: &query,
            is_regex,
            scope,
        };

        let mut total = 0_u32;
        app_state.search_contents(repo, &request, &|| token.is_cancelled(), &mut |batch| {
            total = total.saturating_add(u32::try_from(batch.len()).unwrap_or(u32::MAX));
            let _ = channel.send(SearchChunk::Matches { matches: batch });
        })?;
        Ok(total)
    })
    .await;

    cancellations.finish(id);
    let total = counted?;

    let _ = on_chunk.send(SearchChunk::Done {
        total,
        cancelled: cancel.is_cancelled(),
    });
    Ok(())
}
