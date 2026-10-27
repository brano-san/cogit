use crate::routes::RepoCall;
use crate::{AppState, RepoId, RepoSummary};
use git_engine::{GitError, NetworkStop};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;

const INSIDE_AGENT: &str = "COGIT_INSIDE_AGENT";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Host {
    Loopback(PathBuf),
}

impl Host {
    fn command(&self) -> Command {
        match self {
            Self::Loopback(exe) => {
                let mut command = Command::new(exe);
                command.arg("serve");
                command
            }
        }
    }

    fn agent_path(&self, path: &Path) -> String {
        match self {
            Self::Loopback(_) => path.to_string_lossy().into_owned(),
        }
    }

    fn local_path(&self, path: &str) -> PathBuf {
        match self {
            Self::Loopback(_) => PathBuf::from(path),
        }
    }
}

#[cfg(feature = "loopback")]
pub(crate) fn loopback() -> Option<Host> {
    if std::env::var_os(INSIDE_AGENT).is_some() {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let name = format!("cogit-agent{}", std::env::consts::EXE_SUFFIX);
    exe.ancestors()
        .skip(1)
        .take(2)
        .map(|dir| dir.join(&name))
        .find(|candidate| candidate.is_file())
        .map(Host::Loopback)
}

#[derive(Debug)]
pub(crate) struct Agent {
    host: Host,
    client: Option<agent::Client>,
    child: Child,
}

impl Agent {
    fn start(host: &Host) -> Result<Self, GitError> {
        let mut command = host.command();
        command
            .env(INSIDE_AGENT, "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command.spawn().map_err(|err| unavailable(&err))?;
        let (Some(output), Some(input)) = (child.stdout.take(), child.stdin.take()) else {
            return Err(GitError::Internal("the agent has no pipes".to_owned()));
        };
        let client = agent::Client::connect(output, input).map_err(|err| unavailable(&err))?;
        tracing::info!(?host, version = client.version(), "agent started");
        Ok(Self {
            host: host.clone(),
            client: Some(client),
            child,
        })
    }

    fn ask<T: DeserializeOwned>(
        &self,
        request: &Request,
        stop: Option<&NetworkStop>,
    ) -> Result<T, GitError> {
        let Some(client) = &self.client else {
            return Err(GitError::Internal("the agent is stopping".to_owned()));
        };
        let request = serde_json::to_value(request).map_err(|err| garbled(&err))?;
        match client
            .call(request, stop)
            .map_err(|err| unavailable(&err))?
        {
            Ok(answer) => serde_json::from_value(answer).map_err(|err| garbled(&err)),
            Err(error) => Err(serde_json::from_value(error).unwrap_or_else(|err| garbled(&err))),
        }
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        self.client = None;
        if let Err(err) = self.child.wait() {
            tracing::error!(error = ?err, context = "agent exit");
        }
    }
}

fn unavailable(err: &dyn std::fmt::Display) -> GitError {
    GitError::Io(format!("Cogit's agent is unavailable: {err}"))
}

fn garbled(err: &serde_json::Error) -> GitError {
    GitError::Internal(format!(
        "Cogit's agent sent an answer that does not parse: {err}"
    ))
}

#[derive(Debug, Clone)]
pub(crate) struct Remote {
    agent: Arc<Agent>,
    id: RepoId,
}

impl Remote {
    pub(crate) fn id(&self) -> RepoId {
        self.id
    }

    pub(crate) fn ask<T: DeserializeOwned>(
        &self,
        request: &Request,
        stop: Option<&NetworkStop>,
    ) -> Result<T, GitError> {
        self.agent.ask(request, stop)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum Request {
    Open {
        path: String,
    },
    OpenSubmodule {
        owner: RepoId,
        key: String,
    },
    OpenWorktree {
        owner: RepoId,
        path: String,
    },
    Reread {
        repo: RepoId,
    },
    ConfigFile {
        repo: RepoId,
        scope: git_engine::ConfigScope,
    },
    SaveConfigFile {
        repo: RepoId,
        scope: git_engine::ConfigScope,
        text: String,
        crlf: bool,
    },
    Close {
        repo: RepoId,
    },
    SafetyLog,
    Undo {
        repo: RepoId,
        id: Option<u32>,
    },
    Repo(RepoCall),
}

#[derive(Debug, Serialize, Deserialize)]
struct Opened {
    summary: RepoSummary,
    lane: Option<String>,
}

fn opened(state: &AppState, result: Result<RepoSummary, GitError>) -> Result<Value, Value> {
    answer(result.map(|summary| {
        Opened {
            lane: state
                .get(summary.repo)
                .map(|open| git_engine::slash_path(&open.lane)),
            summary,
        }
    }))
}

pub fn serve_agent(state: &AppState, request: Value, stop: &NetworkStop) -> Result<Value, Value> {
    let request: Request = serde_json::from_value(request).map_err(|err| {
        to_json(&GitError::Internal(format!(
            "not a request of this agent: {err}"
        )))
    })?;
    match request {
        Request::Open { path } => opened(state, state.open_repository(Path::new(&path))),
        Request::OpenSubmodule { owner, key } => opened(state, state.open_submodule(owner, &key)),
        Request::OpenWorktree { owner, path } => opened(state, state.open_worktree(owner, &path)),
        Request::Reread { repo } => answer(state.reread_repository(repo)),
        Request::ConfigFile { repo, scope } => answer(state.config_file(Some(repo), scope)),
        Request::SaveConfigFile {
            repo,
            scope,
            text,
            crlf,
        } => answer(state.save_config_file(Some(repo), scope, &text, crlf)),
        Request::Close { repo } => answer(Ok(state.close_repository(repo))),
        Request::SafetyLog => answer(Ok(state.safety_log())),
        Request::Undo { repo, id } => answer(match id {
            Some(id) => state.undo_entry(repo, id),
            None => state.undo_last(repo),
        }),
        Request::Repo(call) => {
            tracing::debug!(call = call.name(), "agent call");
            call.run(state, stop)
        }
    }
}

pub(crate) fn answer<T: Serialize>(result: Result<T, GitError>) -> Result<Value, Value> {
    result
        .map(|value| to_json(&value))
        .map_err(|err| to_json(&err))
}

fn to_json(value: &impl Serialize) -> Value {
    serde_json::to_value(value).unwrap_or_else(|err| Value::String(err.to_string()))
}

impl AppState {
    pub fn open_repository_on(&self, host: &Host, path: &Path) -> Result<RepoSummary, GitError> {
        let began = self.closes_so_far();
        let agent = self.agent(host)?;
        let request = Request::Open {
            path: host.agent_path(path),
        };
        self.adopt(&agent, &request, None, began)
    }

    pub(crate) fn open_through(
        &self,
        owner: RepoId,
        remote: &Remote,
        request: &Request,
    ) -> Result<RepoSummary, GitError> {
        let began = self.closes_so_far();
        self.adopt(&remote.agent, request, Some(owner), began)
    }

    pub(crate) fn reread_through(
        &self,
        repo: RepoId,
        remote: &Remote,
    ) -> Result<RepoSummary, GitError> {
        let summary: RepoSummary = remote.ask(&Request::Reread { repo: remote.id }, None)?;
        let root = remote.agent.host.local_path(&summary.root);
        Ok(RepoSummary {
            repo,
            root: git_engine::slash_path(&root),
            ..summary
        })
    }

    fn adopt(
        &self,
        agent: &Arc<Agent>,
        request: &Request,
        owner: Option<RepoId>,
        began: u64,
    ) -> Result<RepoSummary, GitError> {
        let Opened { summary, lane } = agent.ask(request, None)?;
        let root = agent.host.local_path(&summary.root);
        let lane = lane.map_or_else(|| root.clone(), |lane| agent.host.local_path(&lane));
        let Some(id) = self.register_remote(root.clone(), summary.name.clone(), owner, lane, began)
        else {
            if let Err(err) = agent.ask::<bool>(&Request::Close { repo: summary.repo }, None) {
                tracing::error!(error = ?err, context = "close in the agent");
            }
            return Err(GitError::InvalidState(format!(
                "{} was closed while it was opening",
                root.display()
            )));
        };
        self.remotes.write().insert(
            id,
            Remote {
                agent: agent.clone(),
                id: summary.repo,
            },
        );
        Ok(RepoSummary {
            repo: id,
            root: git_engine::slash_path(&root),
            ..summary
        })
    }

    #[must_use]
    pub fn is_remote(&self, repo: RepoId) -> bool {
        self.remotes.read().contains_key(&repo)
    }

    pub(crate) fn remote(&self, repo: RepoId) -> Option<Remote> {
        self.remotes.read().get(&repo).cloned()
    }

    pub(crate) fn forget_remote(&self, repo: RepoId) {
        let Some(remote) = self.remotes.write().remove(&repo) else {
            return;
        };
        if let Err(err) = remote.ask::<bool>(&Request::Close { repo: remote.id }, None) {
            tracing::error!(error = ?err, context = "close in the agent");
        }
    }

    pub(crate) fn undo_through(
        &self,
        repo: RepoId,
        remote: &Remote,
        id: Option<u32>,
    ) -> Result<crate::SafetyEntry, GitError> {
        let entry: crate::SafetyEntry = remote.ask(
            &Request::Undo {
                repo: remote.id,
                id,
            },
            None,
        )?;
        Ok(crate::SafetyEntry { repo, ..entry })
    }

    pub(crate) fn remote_safety_log(&self) -> Vec<crate::SafetyEntry> {
        let remotes: Vec<(RepoId, Remote)> = self
            .remotes
            .read()
            .iter()
            .map(|(id, remote)| (*id, remote.clone()))
            .collect();
        let mut asked: Vec<Arc<Agent>> = Vec::new();
        let mut entries = Vec::new();
        for (_, remote) in &remotes {
            if asked.iter().any(|agent| Arc::ptr_eq(agent, &remote.agent)) {
                continue;
            }
            asked.push(remote.agent.clone());
            let log: Vec<crate::SafetyEntry> = match remote.ask(&Request::SafetyLog, None) {
                Ok(log) => log,
                Err(err) => {
                    tracing::error!(error = ?err, context = "safety log of an agent");
                    continue;
                }
            };
            entries.extend(log.into_iter().filter_map(|mut entry| {
                let (id, _) = remotes
                    .iter()
                    .find(|(_, r)| Arc::ptr_eq(&r.agent, &remote.agent) && r.id == entry.repo)?;
                entry.repo = *id;
                Some(entry)
            }));
        }
        entries
    }

    fn agent(&self, host: &Host) -> Result<Arc<Agent>, GitError> {
        let mut agents = self.agents.lock();
        if let Some(agent) = agents.get(host) {
            return Ok(agent.clone());
        }
        let agent = Arc::new(Agent::start(host)?);
        agents.insert(host.clone(), agent.clone());
        Ok(agent)
    }
}
