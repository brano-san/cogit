use crate::backend::{Host, Remote, to_json};
use crate::journal::JOURNAL_CAPACITY;
use crate::rows::RowCache;
use crate::{AppEvent, AppState, CommandNotice, RepoId};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Debug, Serialize, Deserialize)]
enum AgentEvent {
    Changed {
        repo: RepoId,
        kind: fs_watcher::ChangeKind,
    },
    WatchLimited {
        repo: RepoId,
        error: String,
    },
    Command(git_engine::GitOutput),
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum Question {
    Credential { url: String },
    Remember { sign_in: crate::SignIn },
}

pub(crate) fn ask_window<T: serde::de::DeserializeOwned>(
    window: &agent::Events,
    question: &Question,
) -> Option<T> {
    let answer = window.ask(to_json(question))?;
    serde_json::from_value(answer)
        .inspect_err(|err| tracing::error!(error = ?err, context = "the window's answer"))
        .ok()
}

pub(crate) fn answer_agent(
    host: &Host,
    passwords: &crate::credentials::Passwords,
    question: Value,
) -> Value {
    match serde_json::from_value(question) {
        Ok(Question::Credential { url }) => to_json(
            &crate::credentials::stored_in(passwords, &url)
                .filter(|credential| credential.key.is_none() || matches!(host, Host::Loopback(_))),
        ),
        Ok(Question::Remember { sign_in }) => {
            if let Err(err) = crate::credentials::store_in(passwords, &sign_in) {
                tracing::error!(error = ?err, host = %sign_in.host, context = "remembering a sign-in from the agent");
            }
            Value::Null
        }
        Err(err) => {
            tracing::error!(error = ?err, context = "a question of the agent");
            Value::Null
        }
    }
}

pub fn forward_agent_events(state: Arc<AppState>, events: agent::Events) {
    if state.window.set(events.clone()).is_err() {
        tracing::error!(context = "a second window for one agent");
    }
    let commands = events.clone();
    state.tap_commands(Box::new(move |output| {
        commands.send(to_json(&AgentEvent::Command(output.clone())));
    }));
    let mut heard = state.subscribe();
    let forwarding = std::thread::Builder::new()
        .name("agent-events".into())
        .spawn(move || {
            loop {
                let event = match heard.blocking_recv() {
                    Ok(event) => event,
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::error!(skipped, "the agent fell behind its own events");
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => return,
                };
                let told = match event {
                    AppEvent::RepoChanged { repo, kind } => AgentEvent::Changed { repo, kind },
                    AppEvent::WatchLimited { repo, error } => {
                        AgentEvent::WatchLimited { repo, error }
                    }
                    _ => continue,
                };
                if !events.send(to_json(&told)) {
                    return;
                }
            }
        });
    if let Err(err) = forwarding {
        tracing::error!(error = ?err, context = "agent events thread");
    }
}

pub(crate) struct Sink {
    pub(crate) host: Host,
    pub(crate) remotes: Arc<RwLock<HashMap<RepoId, Remote>>>,
    pub(crate) journal: Arc<RwLock<VecDeque<git_engine::GitOutput>>>,
    pub(crate) events: broadcast::Sender<AppEvent>,
    pub(crate) rows: Arc<RwLock<RowCache>>,
}

impl Sink {
    pub(crate) fn take(&self, value: Value) {
        let event: AgentEvent = match serde_json::from_value(value) {
            Ok(event) => event,
            Err(err) => {
                tracing::error!(error = ?err, context = "an event from the agent");
                return;
            }
        };
        let told = match event {
            AgentEvent::Changed { repo, kind } => {
                let Some(repo) = self.ours(repo) else { return };
                self.rows.write().forget(repo);
                AppEvent::RepoChanged { repo, kind }
            }
            AgentEvent::WatchLimited { repo, error } => {
                let Some(repo) = self.ours(repo) else { return };
                AppEvent::WatchLimited { repo, error }
            }
            AgentEvent::Command(mut output) => {
                output.id = git_engine::next_output_id();
                output.repo = self.host.local_path(&output.repo).display().to_string();
                let notice = CommandNotice::from(&output);
                crate::record(&mut self.journal.write(), JOURNAL_CAPACITY, output);
                AppEvent::CommandRecorded(notice)
            }
        };
        let _ = self.events.send(told);
    }

    fn ours(&self, theirs: RepoId) -> Option<RepoId> {
        self.remotes
            .read()
            .iter()
            .find(|(_, remote)| remote.host() == &self.host && remote.id() == theirs)
            .map(|(id, _)| *id)
    }
}
