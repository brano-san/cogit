use crate::registry::RepoRefs;
use crate::{AppState, RepoId, RepoSummary};
use git_engine::{GitError, NetworkStop};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;

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

#[derive(Debug)]
pub(crate) struct Agent {
    client: Option<agent::Client>,
    child: Child,
}

impl Agent {
    fn start(host: &Host) -> Result<Self, GitError> {
        let mut command = host.command();
        command
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
            client: Some(client),
            child,
        })
    }

    fn ask<T: DeserializeOwned>(&self, request: &Request) -> Result<T, GitError> {
        let Some(client) = &self.client else {
            return Err(GitError::Internal("the agent is stopping".to_owned()));
        };
        let request = serde_json::to_value(request).map_err(|err| garbled(&err))?;
        match client
            .call(request, None)
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

macro_rules! routed {
    ($(fn $name:ident($($arg:ident: $ty:ty),*) -> $answer:ty = $local:ident;)*) => {
        #[allow(non_camel_case_types)]
        #[derive(Debug, Serialize, Deserialize)]
        enum RepoCall {
            $($name { repo: RepoId, $($arg: $ty),* },)*
        }

        impl RepoCall {
            fn run(self, state: &AppState) -> Result<Value, Value> {
                match self {
                    $(Self::$name { repo, $($arg),* } => answer(state.$local(repo, $($arg),*)),)*
                }
            }
        }

        impl AppState {
            $(pub fn $name(&self, repo: RepoId, $($arg: $ty),*) -> Result<$answer, GitError> {
                match self.remote(repo) {
                    None => self.$local(repo, $($arg),*),
                    Some(remote) => remote
                        .agent
                        .ask(&Request::Repo(RepoCall::$name { repo: remote.id, $($arg),* })),
                }
            })*
        }
    };
}

routed! {
    fn repo_status() -> git_engine::RepoStatus = local_repo_status;
    fn worktree_files(view: git_engine::WorktreeView) -> git_engine::WorktreeFiles = local_worktree_files;
    fn repo_refs() -> RepoRefs = local_repo_refs;
}

#[derive(Debug, Serialize, Deserialize)]
enum Request {
    Open { path: String },
    Close { repo: RepoId },
    Repo(RepoCall),
}

pub fn serve_agent(state: &AppState, request: Value, _stop: &NetworkStop) -> Result<Value, Value> {
    let request: Request = serde_json::from_value(request).map_err(|err| {
        to_json(&GitError::Internal(format!(
            "not a request of this agent: {err}"
        )))
    })?;
    match request {
        Request::Open { path } => answer(state.open_repository(Path::new(&path))),
        Request::Close { repo } => answer(Ok(state.close_repository(repo))),
        Request::Repo(call) => call.run(state),
    }
}

fn answer<T: Serialize>(result: Result<T, GitError>) -> Result<Value, Value> {
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
        let summary: RepoSummary = agent.ask(&Request::Open {
            path: host.agent_path(path),
        })?;
        let root = host.local_path(&summary.root);
        let Some(id) = self.register_remote(root.clone(), summary.name.clone(), began) else {
            if let Err(err) = agent.ask::<bool>(&Request::Close { repo: summary.repo }) {
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
                agent,
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

    fn remote(&self, repo: RepoId) -> Option<Remote> {
        self.remotes.read().get(&repo).cloned()
    }

    pub(crate) fn forget_remote(&self, repo: RepoId) {
        let Some(remote) = self.remotes.write().remove(&repo) else {
            return;
        };
        if let Err(err) = remote
            .agent
            .ask::<bool>(&Request::Close { repo: remote.id })
        {
            tracing::error!(error = ?err, context = "close in the agent");
        }
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
