mod avatars;
mod bisect;
mod credentials;
pub mod desktop;
mod diffing;
pub mod environment;
mod file_actions;
mod flow;
mod graph_cache;
mod graph_layout;
pub mod graph_overlay;
pub mod graph_wire;
mod handles;
mod hooking;
pub mod investigation;
pub mod licences;
pub mod logging;
mod network;
mod presets;
mod queue;
mod ref_ops;
mod remote_ops;
pub mod repo_rows;
mod rewrite;
mod safety;
pub mod settings;
mod stashing;
pub mod terminal;
mod watching;
mod worktrees;

mod branches;
mod config;
mod conflicts;
mod history;
mod journal;
mod registry;
mod rows;
mod staging;

pub use avatars::{Author, AvatarRow, Avatars};
pub use credentials::{
    KeyringStore, MemoryStore, SecretError, SecretStore, host_of, platform_store,
};
pub use graph_cache::{GraphProgress, GraphWindow};
pub use journal::{CommandNotice, is_warning, record};
pub use network::NetworkRun;
pub use presets::PresetStatus;
pub use queue::{Operation, OperationKind, OperationPermit, OperationPhase, Queue};
pub use registry::{OpenRepo, RepoRefs, RepoSummary, ScanHit};
pub use rows::RepoOverview;
pub use safety::{Recovery, SafetyEntry};

use journal::JOURNAL_CAPACITY;
use parking_lot::RwLock;
use registry::Steps;
use rows::RowCache;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use tokio::sync::broadcast;

const EVENT_CHANNEL_CAPACITY: usize = 256;

/// The next event for a long-lived subscriber. One that fell behind skips what it missed
/// and carries on; only a closed bus ends it.
pub async fn next_event(events: &mut broadcast::Receiver<AppEvent>) -> Option<AppEvent> {
    loop {
        match events.recv().await {
            Ok(event) => return Some(event),
            Err(broadcast::error::RecvError::Lagged(missed)) => {
                tracing::warn!(
                    missed,
                    "event subscriber fell behind; the missed events are skipped"
                );
            }
            Err(broadcast::error::RecvError::Closed) => return None,
        }
    }
}

/// A call for a repository closed since, from a child window or the queue: a normal state,
/// not "not a Git repository" (BE-013).
fn not_open(repo: RepoId) -> git_engine::GitError {
    tracing::info!(repo = repo.0, "a call for a repository no longer open");
    git_engine::GitError::InvalidState(
        "This repository was closed in Cogit. Open it again to go on.".to_owned(),
    )
}

/// The confirmation promised Undo. Without the backup stash there is nothing to undo
/// with, so the destructive step is not taken at all (INV-12).
fn backup_failed(doing: &str, err: &git_engine::GitError) -> git_engine::GitError {
    tracing::error!(error = ?err, context = "backup stash before a destructive step");
    git_engine::GitError::InvalidState(format!(
        "Nothing was changed: the changes could not be saved for Undo before {doing} them. {err}"
    ))
}

fn short(rev: &str) -> &str {
    &rev[..rev.len().min(7)]
}

/// For labels and stash messages: a message is on the command line too, so a thousand
/// names in it would cost the stash that Undo needs (R-191).
fn named(paths: &[String]) -> String {
    match paths {
        [_, _, _, _, ..] => format!("{}, {} and {} more", paths[0], paths[1], paths.len() - 2),
        _ => paths.join(", "),
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
pub struct RepoId(pub u32);

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "type", content = "payload")]
pub enum AppEvent {
    RepoOpened {
        repo: RepoId,
    },
    RepoClosed {
        repo: RepoId,
    },
    RepoChanged {
        repo: RepoId,
        kind: fs_watcher::ChangeKind,
    },
    /// Queued, started or finished — the phase is inside.
    Operation(Operation),
    AvatarReady {
        email: String,
    },
    /// A git command finished. The payload says what happened, not what was printed:
    /// a record can be two megabytes and most of them are never looked at.
    CommandRecorded(CommandNotice),
}

/// What the layout hands `build_graph` as it walks; the webview never sees it (R-193).
#[derive(Debug, Clone)]
pub(crate) struct GraphChunk {
    pub(crate) commits: Vec<git_engine::CommitRow>,
    /// One per commit, in the same order: the node and every segment of its row. Cutting
    /// long links holds the last rows back, so a chunk can have fewer rows than commits.
    pub(crate) rows: Vec<graph_engine::GraphRow>,
    /// Folded merges whose count grew with this chunk.
    pub(crate) folds: Vec<graph_engine::Fold>,
    pub(crate) is_last: bool,
}

pub const DEFAULT_CHUNK_SIZE: usize = 200;

/// What a batch diff came back with. A request the user has already moved on from stops
/// between files rather than finishing work nobody will look at.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DiffBatch {
    Ready {
        files: Vec<diff_engine::FileDiffEntry>,
    },
    /// A newer request for the same repository started while this one was running.
    Superseded,
}

pub struct AppState {
    repos: RwLock<HashMap<RepoId, OpenRepo>>,
    next_repo_id: AtomicU32,
    /// Closes so far, and the count each root was closed at. Written under `repos`: an
    /// open begun before a close of its root must not register it again.
    closes: AtomicU64,
    closed_at: parking_lot::Mutex<HashMap<PathBuf, u64>>,
    events: broadcast::Sender<AppEvent>,
    watchers: Arc<RwLock<HashMap<RepoId, fs_watcher::RepoWatcher>>>,
    /// The one repository a watcher is allowed for (R-351). Locked after `watchers`.
    shown: parking_lot::Mutex<Option<RepoId>>,
    journal: Arc<RwLock<std::collections::VecDeque<git_engine::GitOutput>>>,
    safety: RwLock<Vec<safety::Undoable>>,
    next_entry_id: AtomicU32,
    secrets: Box<dyn SecretStore>,
    pictures: RwLock<Option<Avatars>>,
    preset_dir: RwLock<Option<std::path::PathBuf>>,
    /// The newest diff batch asked for per repository. An older answer never displaces
    /// a newer one, so responses cannot arrive out of order.
    pub(crate) newest_diff: RwLock<HashMap<RepoId, u32>>,
    /// One tree row per repository, good until the watcher or one of our own mutations
    /// says otherwise (problem 5).
    cached_rows: Arc<RwLock<RowCache>>,
    rows_read: Arc<AtomicU32>,
    queue: Queue,
    /// One graph is on screen at a time; a newer request makes the walk before it stop.
    graph_generation: AtomicU32,
    graph: Arc<RwLock<graph_cache::GraphCache>>,
    reachable: parking_lot::Mutex<HashMap<RepoId, git_engine::Reachable>>,
    handles: handles::HandleCache,
    /// The fetch, pull or push running as each queue operation, for `cancel_network`.
    network_runs: parking_lot::Mutex<HashMap<u32, git_engine::NetworkStop>>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("repos", &self.repos)
            .finish_non_exhaustive()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    #[must_use]
    pub fn new() -> Self {
        let (events, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            repos: RwLock::new(HashMap::new()),
            next_repo_id: AtomicU32::new(1),
            closes: AtomicU64::new(0),
            closed_at: parking_lot::Mutex::new(HashMap::new()),
            events,
            watchers: Arc::new(RwLock::new(HashMap::new())),
            shown: parking_lot::Mutex::new(None),
            journal: Arc::new(RwLock::new(std::collections::VecDeque::with_capacity(
                JOURNAL_CAPACITY,
            ))),
            safety: RwLock::new(Vec::new()),
            next_entry_id: AtomicU32::new(1),
            secrets: platform_store(),
            pictures: RwLock::new(None),
            preset_dir: RwLock::new(None),
            newest_diff: RwLock::new(HashMap::new()),
            cached_rows: Arc::new(RwLock::new(RowCache::default())),
            rows_read: Arc::new(AtomicU32::new(0)),
            queue: Queue::default(),
            graph_generation: AtomicU32::new(0),
            graph: Arc::new(RwLock::new(graph_cache::GraphCache::default())),
            reachable: parking_lot::Mutex::new(HashMap::new()),
            handles: handles::HandleCache::default(),
            network_runs: parking_lot::Mutex::new(HashMap::new()),
        }
    }

    /// Ticking a hundred tags is one request, and ticking again mid-walk retires the old
    /// one: its next chunk sees it is no longer current and the walk ends there.
    pub fn begin_graph(&self) -> u32 {
        self.graph_generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    #[must_use]
    pub fn is_current_graph(&self, generation: u32) -> bool {
        self.graph_generation.load(Ordering::SeqCst) == generation
    }

    /// Turning avatars on is what creates the cache directory; `off` never gets here.
    pub fn enable_avatars(&self, dir: std::path::PathBuf) -> Result<(), ::avatars::CacheError> {
        self.enable_avatars_with(dir, std::sync::Arc::new(::avatars::Gravatar::new()))
    }

    pub fn enable_avatars_with<S: ::avatars::Source>(
        &self,
        dir: std::path::PathBuf,
        source: std::sync::Arc<S>,
    ) -> Result<(), ::avatars::CacheError> {
        let service = Avatars::new(dir, source, self.events.clone())?;
        let old = self.pictures.write().replace(service);
        drop(old);
        Ok(())
    }

    /// Dropping the service waits for the downloads in flight, so it happens after the lock
    /// every avatar read takes is released, and never on an async worker (the command
    /// calls it through `blocking`).
    pub fn disable_avatars(&self) {
        let old = self.pictures.write().take();
        drop(old);
    }

    /// The addresses on screen, for the download queue. Reads nothing.
    pub fn avatar_window(&self, emails: &[String]) {
        if let Some(service) = self.pictures.read().as_ref() {
            service.window(emails);
        }
    }

    /// The pictures these authors have, if any. The answer is immediate.
    #[must_use]
    pub fn avatars(&self, authors: &[Author]) -> Vec<AvatarRow> {
        match self.pictures.read().as_ref() {
            Some(service) => service.rows(authors),
            None => avatars::rows_without_pictures(authors),
        }
    }

    /// Tests only: waits for the queue to settle so an assertion is not a race.
    pub fn drain_avatars(&self) {
        if let Some(service) = self.pictures.read().as_ref() {
            service.drain();
        }
    }

    /// Only ever answers whether a token exists: the value must not reach the webview.
    #[must_use]
    pub fn has_token(&self, host: &str) -> bool {
        self.secrets.get(host).is_some()
    }

    pub fn store_token(&self, host: &str, token: &str) -> Result<(), SecretError> {
        self.secrets.set(host, token)
    }

    pub fn forget_token(&self, host: &str) -> Result<(), SecretError> {
        self.secrets.delete(host)
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.events.subscribe()
    }

    pub fn emit(&self, event: AppEvent) {
        let _ = self.events.send(event);
    }

    fn handle(&self, repo: RepoId) -> Result<git_engine::RepoHandle, git_engine::GitError> {
        let open = self.get(repo).ok_or_else(|| not_open(repo))?;
        Ok(self
            .handles
            .handle(repo, &open.root)?
            .with_journal(self.command_sink(repo)))
    }

    /// How many times a command had to open its repository from disk.
    #[must_use]
    pub fn repositories_opened(&self) -> u32 {
        self.handles.opened()
    }

    /// Open repositories kept for the next command.
    #[must_use]
    pub fn repositories_held(&self) -> usize {
        self.handles.held()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribers_receive_lifecycle_events() {
        let state = AppState::new();
        let mut rx = state.subscribe();
        let id = state.register(PathBuf::from("/a"), "a".into());
        match rx.try_recv() {
            Ok(AppEvent::RepoOpened { repo }) => assert_eq!(repo, id),
            other => panic!("expected RepoOpened, got {other:?}"),
        }
    }

    #[test]
    fn emitting_without_subscribers_is_not_an_error() {
        let state = AppState::new();
        state.emit(AppEvent::Operation(Operation {
            id: 1,
            repo: None,
            kind: OperationKind::Fetch,
            label: "fetch".into(),
            phase: OperationPhase::Running,
            success: None,
        }));
    }
}
