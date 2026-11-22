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
    Credential {
        url: String,
    },
    Remember {
        sign_in: crate::SignIn,
    },
    HostKey {
        question: git_engine::HostKeyQuestion,
    },
}

pub(crate) fn ask_window<T: serde::de::DeserializeOwned>(
    window: &agent::Events,
    question: &Question,
) -> Option<T> {
    window.ask(to_json(question)).and_then(read_answer)
}

fn read_answer<T: serde::de::DeserializeOwned>(answer: Value) -> Option<T> {
    serde_json::from_value(answer)
        .inspect_err(|err| tracing::error!(error = ?err, context = "the window's answer"))
        .ok()
}

pub(crate) fn answer_agent(
    host: &Host,
    passwords: &crate::credentials::Passwords,
    host_keys: &crate::host_keys::HostKeyDesk,
    question: Value,
    withdrawn: &dyn Fn() -> bool,
) -> Value {
    if matches!(host, Host::Ssh { .. }) {
        return Value::Null;
    }
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
        Ok(Question::HostKey { question }) => {
            to_json(&host_keys.ask(&question, withdrawn, git_engine::HOST_KEY_ASK_LIMIT))
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
    if !crate::ssh_transport::inside_agent() {
        let window = events.clone();
        git_engine::ask_host_keys(Arc::new(move |question, gone| {
            let question = Question::HostKey {
                question: question.clone(),
            };
            window
                .ask_until(to_json(&question), gone)
                .and_then(read_answer::<bool>)
                .unwrap_or(false)
        }));
    }
    let commands = events.clone();
    state.tap_commands(Box::new(move |output| {
        let mut output = output.clone();
        output.answered |= crate::network::asks_sign_in(output.credential_trouble);
        commands.send(to_json(&AgentEvent::Command(output)));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ssh_agent_cannot_read_or_replace_desktop_credentials() {
        let state = AppState::new();
        let directory = tempfile::tempdir().unwrap();
        state.use_password_store(directory.path().join("passwords.json"));
        state.create_credential_store(None).unwrap();
        let mut login = crate::SignIn {
            host: "example.test".into(),
            username: Some("user".into()),
            secret: "original".into(),
            key_path: None,
            remember: true,
        };
        state.store_credential(&login).unwrap();
        let remote = Host::of(std::path::Path::new("ssh://box/project")).unwrap();
        let ask = |question| {
            answer_agent(
                &remote,
                &state.passwords,
                &state.host_keys,
                to_json(&question),
                &|| false,
            )
        };
        let url = "https://example.test/repo";
        assert_eq!(ask(Question::Credential { url: url.into() }), Value::Null);
        login.secret = "replacement".into();
        assert_eq!(ask(Question::Remember { sign_in: login }), Value::Null);
        assert_eq!(state.stored_credential(url).unwrap().secret, "original");
    }
}
