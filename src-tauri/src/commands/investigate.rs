//! The Investigate window (#15): the file's log, blame with origins, origin candidates.

use app_state::RepoId;
use git_engine::{
    BlameCommit, BlameSource, FileRevision, GitError, OriginLine, OriginQuery, OriginReport,
};
use serde::Serialize;

use super::blocking;

const CHUNK: usize = 200;

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum BlameChunk {
    /// Always first: the lines that follow index into these tables.
    Header {
        commits: Vec<BlameCommit>,
        sources: Vec<BlameSource>,
    },
    Lines {
        lines: Vec<OriginLine>,
    },
}

/// `Started` carries the id `cancel_operation` needs, long before the answer.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum OriginEvent {
    Started { id: u32 },
    Done { report: OriginReport },
    Cancelled,
}

/// Every commit that changed the file, newest first, from `rev` (HEAD when absent).
#[tauri::command]
#[specta::specta]
pub async fn investigate_log(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    rev: Option<String>,
    follow: bool,
    on_chunk: tauri::ipc::Channel<Vec<FileRevision>>,
) -> Result<u32, GitError> {
    let app_state = state.state.clone();
    let log = blocking("investigate_log", move || {
        app_state.file_log(repo, &path, rev.as_deref(), follow)
    })
    .await?;
    for chunk in log.chunks(CHUNK) {
        let _ = on_chunk.send(chunk.to_vec());
    }
    Ok(u32::try_from(log.len()).unwrap_or(u32::MAX))
}

/// The file at `rev` (the working tree when absent), each line with its origin.
#[tauri::command]
#[specta::specta]
pub async fn investigate_blame(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    path: String,
    rev: Option<String>,
    ignore_whitespace: bool,
    on_chunk: tauri::ipc::Channel<BlameChunk>,
) -> Result<u32, GitError> {
    let app_state = state.state.clone();
    let started = std::time::Instant::now();
    let report = blocking("investigate_blame", move || {
        app_state.blame_origins(repo, &path, rev.as_deref(), ignore_whitespace)
    })
    .await?;
    tracing::debug!(
        repo = repo.0,
        lines = report.lines.len(),
        commits = report.commits.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "blame with origins computed"
    );

    let total = u32::try_from(report.lines.len()).unwrap_or(u32::MAX);
    let _ = on_chunk.send(BlameChunk::Header {
        commits: report.commits,
        sources: report.sources,
    });
    for chunk in report.lines.chunks(CHUNK) {
        let _ = on_chunk.send(BlameChunk::Lines {
            lines: chunk.to_vec(),
        });
    }
    Ok(total)
}

/// Searches in the background where a block came from; `cancel_operation` stops it.
#[tauri::command]
#[specta::specta]
pub async fn origin_candidates(
    state: tauri::State<'_, crate::AppContext>,
    cancellations: tauri::State<'_, std::sync::Arc<crate::operations::Cancellations>>,
    repo: RepoId,
    query: OriginQuery,
    on_event: tauri::ipc::Channel<OriginEvent>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    let cancellations = std::sync::Arc::clone(&cancellations);
    let (id, cancel) = cancellations.start();
    let _ = on_event.send(OriginEvent::Started { id });

    let started = std::time::Instant::now();
    let token = cancel.clone();
    let found = blocking("origin_candidates", move || {
        app_state.origin_candidates(repo, &query, &|| token.is_cancelled())
    })
    .await;
    cancellations.finish(id);

    let event = match found? {
        Some(report) => {
            tracing::debug!(
                candidates = report.candidates.len(),
                elapsed_ms = started.elapsed().as_millis(),
                "origin candidates found"
            );
            OriginEvent::Done { report }
        }
        None => OriginEvent::Cancelled,
    };
    let _ = on_event.send(event);
    Ok(())
}

/// `async`, unlike the older window commands: WebView2 can deadlock when a window is
/// built on the thread that delivered a synchronous command (R-283).
#[tauri::command]
#[specta::specta]
pub async fn open_investigate_window(
    app: tauri::AppHandle,
    url: String,
    title: String,
) -> Result<(), GitError> {
    crate::child_window::open(
        &app,
        "investigate",
        url,
        title,
        crate::child_window::Shape {
            width: 1280.0,
            height: 860.0,
            min_width: 820.0,
            min_height: 560.0,
        },
    )
    .map_err(|err| GitError::Internal(format!("cannot open the Investigate window: {err}")))
}
