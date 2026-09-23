//! The Investigate window (#15): the file's log, blame with origins, origin candidates.

use app_state::RepoId;
use git_engine::{
    BlameCommit, BlameSource, FileRevision, GitError, OriginLine, OriginQuery, OriginReport,
};
use serde::Serialize;

use super::blocking;
use crate::child_window::{CLOSE, Item, Submenu};

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

/// The Investigate window with its own menu. Building a window blocks, so it happens on
/// the blocking pool, never on the thread that delivers IPC (R-201, R-283).
#[tauri::command]
#[specta::specta]
pub async fn open_investigate_window(
    app: tauri::AppHandle,
    url: String,
    title: String,
) -> Result<(), GitError> {
    blocking("open_investigate_window", move || {
        crate::child_window::open_with_menu(
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
            MENU,
        )
        .map_err(|err| GitError::Internal(format!("cannot open the Investigate window: {err}")))
    })
    .await
}

const fn item(
    action: &'static str,
    label: &'static str,
    accelerator: Option<&'static str>,
) -> Item {
    Item {
        action,
        label,
        accelerator,
    }
}

/// File, Edit, View, Go To, Window, Help — DeepGit's bar. Every action but Close reaches
/// the page as a `cogit-menu` event (`lib/investigate/menu.ts`).
pub const MENU: &[Submenu] = &[
    Submenu {
        title: "File",
        items: &[item(CLOSE, "Close", Some("CmdOrCtrl+W"))],
    },
    Submenu {
        title: "Edit",
        items: &[
            item("copy-line", "Copy Line", None),
            item(
                "copy-commit-id",
                "Copy Commit ID",
                Some("CmdOrCtrl+Shift+C"),
            ),
            item("copy-path", "Copy File Path", None),
        ],
    },
    Submenu {
        title: "View",
        items: &[
            item("follow-renames", "Follow Renames (on/off)", None),
            item(
                "ignore-whitespace",
                "Ignore Whitespace Changes (on/off)",
                None,
            ),
            item("refresh", "Refresh", Some("F5")),
        ],
    },
    Submenu {
        title: "Go To",
        items: &[
            item("back", "Back", Some("Alt+Left")),
            item("forward", "Forward", Some("Alt+Right")),
            item("go-deeper", "Go Deeper", Some("CmdOrCtrl+D")),
            item("close-card", "Hide Origin Card", None),
            item("previous-change", "Previous Change", Some("Shift+F6")),
            item("next-change", "Next Change", Some("F6")),
            item("newer-version", "Newer Version", Some("Alt+Up")),
            item("older-version", "Older Version", Some("Alt+Down")),
        ],
    },
    Submenu {
        title: "Window",
        items: &[
            item("perspective-log", "Log", Some("CmdOrCtrl+1")),
            item("perspective-diff", "Diff", Some("CmdOrCtrl+2")),
            item("perspective-blame", "Blame", Some("CmdOrCtrl+3")),
            item(
                "perspective-blame-origins",
                "Blame+Origins",
                Some("CmdOrCtrl+4"),
            ),
            item("perspective-origins", "Origins", Some("CmdOrCtrl+5")),
        ],
    },
    Submenu {
        title: "Help",
        items: &[item("help", "How Investigate Works", Some("F1"))],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> impl Iterator<Item = &'static Item> {
        MENU.iter().flat_map(|submenu| submenu.items.iter())
    }

    #[test]
    fn the_bar_has_the_six_menus_the_window_promises() {
        let titles: Vec<&str> = MENU.iter().map(|submenu| submenu.title).collect();
        assert_eq!(titles, ["File", "Edit", "View", "Go To", "Window", "Help"]);
    }

    /// An action becomes a JavaScript string literal and a DOM event's detail.
    #[test]
    fn actions_are_plain_and_distinct() {
        let actions: Vec<&str> = items().map(|item| item.action).collect();
        assert!(
            actions
                .iter()
                .all(|action| action.chars().all(|c| c.is_ascii_lowercase() || c == '-'))
        );
        let mut distinct = actions.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), actions.len());
    }

    #[test]
    fn no_two_items_share_a_shortcut() {
        let mut keys: Vec<&str> = items().filter_map(|item| item.accelerator).collect();
        let all = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), all);
    }

    #[test]
    fn the_window_closes_from_its_menu_with_ctrl_w() {
        assert!(
            items().any(|item| item.action == CLOSE && item.accelerator == Some("CmdOrCtrl+W"))
        );
    }
}
