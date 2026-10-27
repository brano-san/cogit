use crate::location::RepoLocation;
use crate::routes::RepoCall;
use crate::{AppState, RepoId, RepoSummary};
use git_engine::{GitError, NetworkStop};
use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;

const INSIDE_AGENT: &str = "COGIT_INSIDE_AGENT";
const AGENT_IN_DISTRO: &str = "COGIT_AGENT";
const NOT_INSTALLED: i32 = 97;
const LAST_WORDS: usize = 40;

static AGENT_BINARY: OnceLock<&'static [u8]> = OnceLock::new();
static AGENT_DIR: OnceLock<String> = OnceLock::new();

pub fn provide_agent_binary(bytes: &'static [u8]) {
    if !bytes.is_empty() && AGENT_BINARY.set(bytes).is_err() {
        tracing::error!(context = "the agent binary was provided twice");
    }
}

fn agent_dir(bytes: &[u8]) -> &'static str {
    AGENT_DIR.get_or_init(|| {
        use std::hash::{Hash as _, Hasher as _};
        let mut hasher = std::hash::DefaultHasher::new();
        bytes.hash(&mut hasher);
        format!(
            "{}-{:08x}",
            env!("CARGO_PKG_VERSION"),
            hasher.finish() & 0xffff_ffff
        )
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Host {
    Loopback(PathBuf),
    Wsl { distro: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum BackendState {
    Starting,
    Started,
    Failed,
    Lost { reason: String },
}

impl Host {
    #[must_use]
    pub fn of(path: &Path) -> Option<Self> {
        match RepoLocation::of(path) {
            RepoLocation::Wsl { distro, .. } => Some(Self::Wsl { distro }),
            RepoLocation::Local | RepoLocation::Ssh { .. } => None,
        }
    }

    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Loopback(_) => "loopback".to_owned(),
            Self::Wsl { distro } => format!("WSL: {distro}"),
        }
    }

    fn command(&self) -> Result<Command, GitError> {
        match self {
            Self::Loopback(exe) => {
                let mut command = Command::new(exe);
                command.arg("serve");
                Ok(command)
            }
            Self::Wsl { distro } => {
                let mut command = Command::new("wsl.exe");
                command.args(["-d", distro, "-e", "sh", "-c"]);
                if let Some(agent) = std::env::var_os(AGENT_IN_DISTRO) {
                    command.args([r#"exec "$1" serve"#, "cogit-agent"]);
                    command.arg(agent);
                    return Ok(command);
                }
                let Some(bytes) = AGENT_BINARY.get() else {
                    return Err(GitError::Unavailable(format!(
                        "This build of Cogit carries no agent, so it cannot open repositories in {}. \
                         Build it with COGIT_AGENT_BIN, or set {AGENT_IN_DISTRO} to an agent inside the distribution.",
                        self.label()
                    )));
                };
                command.args([
                    &format!(
                        r#"a="$HOME/.cache/cogit/agent/$1/cogit-agent"; [ -x "$a" ] || exit {NOT_INSTALLED}; exec "$a" serve"#
                    ),
                    "cogit-agent",
                    agent_dir(bytes),
                ]);
                Ok(command)
            }
        }
    }

    fn install(&self) -> Result<bool, GitError> {
        let (Self::Wsl { distro }, Some(bytes)) = (self, AGENT_BINARY.get()) else {
            return Ok(false);
        };
        let started = std::time::Instant::now();
        let mut command = Command::new("wsl.exe");
        command
            .args(["-d", distro, "-e", "sh", "-c"])
            .arg(concat!(
                r#"set -e; d="$HOME/.cache/cogit/agent"; mkdir -p "$d/$1"; t="$d/$1/.part.$$"; "#,
                r#"cat > "$t"; chmod 755 "$t"; mv -f "$t" "$d/$1/cogit-agent"; cd "$d"; "#,
                r#"ls -1t | tail -n +3 | while IFS= read -r old; do rm -rf -- "$old"; done"#
            ))
            .args(["cogit-install", agent_dir(bytes)])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        hide_window(&mut command);
        let mut child = command.spawn().map_err(|err| self.unreachable(&err))?;
        let written = child.stdin.take().map(|mut input| input.write_all(bytes));
        let output = child
            .wait_with_output()
            .map_err(|err| self.unreachable(&err))?;
        if !output.status.success() || !matches!(written, Some(Ok(()))) {
            return Err(GitError::Unavailable(format!(
                "Cogit could not install its agent in {}: {}\n{}",
                self.label(),
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        tracing::info!(host = ?self, elapsed_ms = started.elapsed().as_millis(), "agent installed");
        Ok(true)
    }

    fn agent_path(&self, path: &Path) -> String {
        match (self, RepoLocation::of(path)) {
            (Self::Wsl { .. }, RepoLocation::Wsl { path, .. }) => path,
            _ => path.to_string_lossy().into_owned(),
        }
    }

    fn source_path(&self, source: &str) -> String {
        let windows_drive = source.as_bytes().get(1) == Some(&b':')
            && source
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic());
        match self {
            Self::Wsl { .. } if Host::of(Path::new(source)).as_ref() == Some(self) => {
                self.agent_path(Path::new(source))
            }
            Self::Wsl { .. } if windows_drive => format!(
                "/mnt/{}{}",
                source[..1].to_ascii_lowercase(),
                source[2..].replace('\\', "/")
            ),
            _ => source.to_owned(),
        }
    }

    pub(crate) fn local_path(&self, path: &str) -> PathBuf {
        match self {
            Self::Loopback(_) => PathBuf::from(path),
            Self::Wsl { distro } => PathBuf::from(format!(
                r"\\wsl.localhost\{distro}{}",
                path.replace('/', r"\")
            )),
        }
    }

    fn unreachable(&self, err: &dyn std::fmt::Display) -> GitError {
        GitError::Unavailable(format!("Cogit could not reach {}: {err}", self.label()))
    }
}

fn hide_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = command;
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

#[derive(Debug, Default)]
struct LastWords {
    lines: VecDeque<String>,
    exit: Option<String>,
}

impl LastWords {
    fn told(&self) -> String {
        let mut text = self.exit.clone().unwrap_or_default();
        if !self.lines.is_empty() {
            text.push_str("\nIts last output:\n");
            text.push_str(&Vec::from(self.lines.clone()).join("\n"));
        }
        text
    }
}

struct Spawned {
    child: Child,
    words: Arc<Mutex<LastWords>>,
    listener: JoinHandle<()>,
}

impl Spawned {
    fn finish(mut self) -> (Option<i32>, String) {
        let status = self.child.wait();
        if self.listener.join().is_err() {
            tracing::error!(context = "agent stderr reader panicked");
        }
        let mut words = self.words.lock();
        let code = status
            .as_ref()
            .ok()
            .and_then(std::process::ExitStatus::code);
        words.exit = Some(match status {
            Ok(status) => format!("The agent exited: {status}."),
            Err(err) => format!("The agent's exit is unknown: {err}."),
        });
        (code, words.told())
    }
}

type Failure = (Option<i32>, GitError);

#[derive(Debug)]
pub(crate) struct Agent {
    host: Host,
    pid: u32,
    client: Option<agent::Client>,
    leaving: Arc<AtomicBool>,
    words: Arc<Mutex<LastWords>>,
}

impl Agent {
    fn start(
        host: &Host,
        on_event: impl Fn(Value) + Send + Clone + 'static,
        on_ask: impl Fn(Value) -> Value + Send + Sync + Clone + 'static,
        on_lost: impl FnOnce(String) + Send + 'static,
    ) -> Result<Self, GitError> {
        let (client, spawned) = match Self::connect(host, on_event.clone(), on_ask.clone()) {
            Err((Some(NOT_INSTALLED), _)) if host.install()? => {
                Self::connect(host, on_event, on_ask)
            }
            other => other,
        }
        .map_err(|(_, err)| err)?;
        Ok(Self::watch(host, client, spawned, on_lost))
    }

    fn connect(
        host: &Host,
        on_event: impl Fn(Value) + Send + 'static,
        on_ask: impl Fn(Value) -> Value + Send + Sync + 'static,
    ) -> Result<(agent::Client, Spawned), Failure> {
        let mut command = host.command().map_err(|err| (None, err))?;
        command
            .env(INSIDE_AGENT, "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        hide_window(&mut command);
        let mut child = command
            .spawn()
            .map_err(|err| (None, host.unreachable(&err)))?;
        let (Some(output), Some(input), Some(errors)) =
            (child.stdout.take(), child.stdin.take(), child.stderr.take())
        else {
            return Err((
                None,
                GitError::Internal("the agent has no pipes".to_owned()),
            ));
        };
        let words = Arc::new(Mutex::new(LastWords::default()));
        let heard = Arc::clone(&words);
        let label = host.label();
        let listener = std::thread::spawn(move || {
            for line in BufReader::new(errors).lines() {
                let Ok(line) = line else { break };
                tracing::warn!(host = label, line, "agent stderr");
                let mut words = heard.lock();
                if words.lines.len() == LAST_WORDS {
                    words.lines.pop_front();
                }
                words.lines.push_back(line);
            }
        });
        let spawned = Spawned {
            child,
            words,
            listener,
        };
        match agent::Client::connect_with(output, input, on_event, on_ask) {
            Ok(client) => Ok((client, spawned)),
            Err(err) => {
                let (code, told) = spawned.finish();
                Err((
                    code,
                    GitError::Unavailable(format!(
                        "Cogit's agent in {} did not start: {err}\n{told}",
                        host.label()
                    )),
                ))
            }
        }
    }

    fn watch(
        host: &Host,
        client: agent::Client,
        spawned: Spawned,
        on_lost: impl FnOnce(String) + Send + 'static,
    ) -> Self {
        tracing::info!(?host, version = client.version(), "agent started");
        let leaving = Arc::new(AtomicBool::new(false));
        let words = Arc::clone(&spawned.words);
        let pid = spawned.child.id();
        let left = Arc::clone(&leaving);
        std::thread::spawn(move || {
            let (_, told) = spawned.finish();
            if !left.load(Ordering::Acquire) {
                tracing::error!(reason = told, "agent lost");
                on_lost(told);
            }
        });
        Self {
            host: host.clone(),
            pid,
            client: Some(client),
            leaving,
            words,
        }
    }

    fn is_gone(&self) -> bool {
        self.client.as_ref().is_none_or(agent::Client::is_gone)
    }

    fn ask<T: DeserializeOwned>(
        &self,
        request: &Request,
        stop: Option<&NetworkStop>,
    ) -> Result<T, GitError> {
        self.stream(
            request,
            &|| stop.is_some_and(NetworkStop::is_stopped),
            &mut |_| {},
        )
    }

    fn stream<T: DeserializeOwned>(
        &self,
        request: &Request,
        canceled: &dyn Fn() -> bool,
        on_chunk: &mut dyn FnMut(Value),
    ) -> Result<T, GitError> {
        let Some(client) = &self.client else {
            return Err(GitError::Internal("the agent is stopping".to_owned()));
        };
        let request = serde_json::to_value(request).map_err(|err| garbled(&err))?;
        match client
            .call_streaming(request, canceled, on_chunk)
            .map_err(|err| self.lost(&err))?
        {
            Ok(answer) => serde_json::from_value(answer).map_err(|err| garbled(&err)),
            Err(error) => Err(serde_json::from_value(error).unwrap_or_else(|err| garbled(&err))),
        }
    }

    fn lost(&self, err: &agent::AgentError) -> GitError {
        GitError::Unavailable(format!(
            "Cogit's agent in {} stopped: {err}\n{}\nRefresh (F5) starts it again.",
            self.host.label(),
            self.words.lock().told()
        ))
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        self.leaving.store(true, Ordering::Release);
        self.client = None;
    }
}

fn check_git(host: &Host, agent: &Agent) -> Result<(), GitError> {
    let version: String = agent.ask(&Request::GitVersion, None).map_err(|err| {
        GitError::Unavailable(format!(
            "Git is not installed in {}, and Cogit needs it there: sudo apt install git\n{err}",
            host.label()
        ))
    })?;
    let version = version.trim_start_matches("git version ").to_owned();
    if git_engine::is_below_min_git(&version) {
        tracing::warn!(host = ?host, version, "git in the distribution is older than Cogit is tested with");
    }
    tracing::info!(host = ?host, version, "git of the agent");
    Ok(())
}

fn linux_trash(paths: &[PathBuf]) -> std::io::Result<()> {
    let platform = crate::desktop::Platform::current();
    let Some(launch) = crate::desktop::trash_command(platform, paths) else {
        return Err(std::io::Error::other("there is no bin on this system"));
    };
    crate::desktop::run(&launch).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            std::io::Error::other(
                "gio is not installed, so there is no bin to delete to: sudo apt install libglib2.0-bin",
            )
        } else {
            err
        }
    })
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

    pub(crate) fn host(&self) -> &Host {
        &self.agent.host
    }

    pub(crate) fn stream<T: DeserializeOwned>(
        &self,
        request: &Request,
        canceled: &dyn Fn() -> bool,
        on_chunk: &mut dyn FnMut(Value),
    ) -> Result<T, GitError> {
        self.agent.stream(request, canceled, on_chunk)
    }

    pub(crate) fn ask<T: DeserializeOwned>(
        &self,
        request: &Request,
        stop: Option<&NetworkStop>,
    ) -> Result<T, GitError> {
        self.agent.ask(request, stop)
    }

    pub(crate) fn call<T: DeserializeOwned>(
        &self,
        request: &Request,
        stop: Option<&NetworkStop>,
        on_line: Option<&mut dyn FnMut(&str)>,
    ) -> Result<T, GitError> {
        let canceled = || stop.is_some_and(NetworkStop::is_stopped);
        match on_line {
            None => self.agent.stream(request, &canceled, &mut |_| {}),
            Some(on_line) => self.agent.stream(request, &canceled, &mut |chunk| {
                if let Value::String(line) = chunk {
                    on_line(&line);
                }
            }),
        }
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
    Trash {
        repo: RepoId,
        paths: Vec<String>,
    },
    Overview {
        repo: RepoId,
    },
    Clone {
        request: git_engine::CloneRequest,
        sign_in: Option<crate::SignIn>,
    },
    Init {
        path: String,
    },
    GitVersion,
    Show {
        repo: Option<RepoId>,
    },
    GraphBudget {
        bytes: usize,
    },
    Stream(crate::streams::StreamCall),
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

pub fn serve_agent(state: &AppState, request: Value, call: &agent::Call) -> Result<Value, Value> {
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
        Request::Clone { request, sign_in } => answer(
            state
                .clone_repository(&request, sign_in.as_ref(), call.stop(), |line| {
                    call.chunk(Value::String(line.to_owned()));
                })
                .map(|done| git_engine::slash_path(&done)),
        ),
        Request::GitVersion => answer(git_engine::git_version()),
        Request::Init { path } => answer(
            state
                .init_repository(Path::new(&path))
                .map(|done| git_engine::slash_path(&done)),
        ),
        Request::Trash { repo, paths } => answer(state.move_to_trash(repo, &paths, linux_trash)),
        Request::Overview { repo } => answer(
            state
                .get(repo)
                .map(|open| state.local_overview_of(&open))
                .ok_or_else(|| crate::not_open(repo)),
        ),
        Request::Show { repo } => {
            state.show_repository(repo);
            answer(Ok(()))
        }
        Request::GraphBudget { bytes } => {
            state.set_graph_cache_budget(bytes);
            answer(Ok(()))
        }
        Request::Stream(stream) => stream.run(state, call),
        Request::Undo { repo, id } => answer(match id {
            Some(id) => state.undo_entry(repo, id),
            None => state.undo_last(repo),
        }),
        Request::Repo(repo_call) => {
            tracing::debug!(call = repo_call.name(), "agent call");
            repo_call.run(state, call)
        }
    }
}

pub(crate) fn answer<T: Serialize>(result: Result<T, GitError>) -> Result<Value, Value> {
    result
        .map(|value| to_json(&value))
        .map_err(|err| to_json(&err))
}

pub(crate) fn to_json(value: &impl Serialize) -> Value {
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
        let remote = if remote.agent.is_gone() {
            &self.reconnect(repo, remote)?
        } else {
            remote
        };
        let summary: RepoSummary = remote.ask(&Request::Reread { repo: remote.id }, None)?;
        let root = remote.agent.host.local_path(&summary.root);
        Ok(RepoSummary {
            repo,
            root: git_engine::slash_path(&root),
            ..summary
        })
    }

    fn reconnect(&self, repo: RepoId, remote: &Remote) -> Result<Remote, GitError> {
        let root = self.root_of(repo)?;
        let agent = self.agent(&remote.agent.host)?;
        let Opened { summary, .. } = agent.ask(
            &Request::Open {
                path: agent.host.agent_path(&root),
            },
            None,
        )?;
        let fresh = Remote {
            agent,
            id: summary.repo,
        };
        self.remotes.write().insert(repo, fresh.clone());
        tracing::info!(repo = repo.0, root = %root.display(), "reconnected to the agent");
        Ok(fresh)
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

    #[doc(hidden)]
    #[must_use]
    pub fn agent_process_id(&self, repo: RepoId) -> Option<u32> {
        self.remote(repo).map(|remote| remote.agent.pid)
    }

    #[must_use]
    pub fn is_remote(&self, repo: RepoId) -> bool {
        self.remotes.read().contains_key(&repo)
    }

    pub(crate) fn budget_on_agents(&self, bytes: usize) {
        let agents: Vec<Arc<Agent>> = self.agents.lock().values().cloned().collect();
        for agent in agents {
            if let Err(err) = agent.ask::<()>(&Request::GraphBudget { bytes }, None) {
                tracing::error!(error = ?err, context = "graph budget in the agent");
            }
        }
    }

    pub(crate) fn show_on_agents(&self, shown: Option<RepoId>) {
        let target = shown.and_then(|repo| self.remote(repo));
        let agents: Vec<Arc<Agent>> = self.agents.lock().values().cloned().collect();
        for agent in agents {
            let repo = target
                .as_ref()
                .filter(|remote| Arc::ptr_eq(&remote.agent, &agent))
                .map(|remote| remote.id);
            if let Err(err) = agent.ask::<()>(&Request::Show { repo }, None) {
                tracing::error!(error = ?err, context = "show in the agent");
            }
        }
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

    pub(crate) fn clone_through(
        &self,
        host: &Host,
        request: &git_engine::CloneRequest,
        sign_in: Option<&crate::SignIn>,
        stop: &NetworkStop,
        mut on_line: impl FnMut(&str),
    ) -> Result<PathBuf, GitError> {
        let agent = self.agent(host)?;
        let request = Request::Clone {
            request: git_engine::CloneRequest {
                source: host.source_path(request.source.trim()),
                target: host.agent_path(Path::new(request.target.trim())),
                ..request.clone()
            },
            sign_in: sign_in.cloned(),
        };
        let done: String = agent.stream(&request, &|| stop.is_stopped(), &mut |chunk| {
            if let Value::String(line) = chunk {
                on_line(&line);
            }
        })?;
        Ok(host.local_path(&done))
    }

    pub(crate) fn init_through(&self, host: &Host, path: &Path) -> Result<PathBuf, GitError> {
        let done: String = self.agent(host)?.ask(
            &Request::Init {
                path: host.agent_path(path),
            },
            None,
        )?;
        Ok(host.local_path(&done))
    }

    pub(crate) fn overview_through(
        &self,
        open: &crate::OpenRepo,
        remote: &Remote,
    ) -> crate::RepoOverview {
        match remote.ask::<crate::RepoOverview>(&Request::Overview { repo: remote.id }, None) {
            Ok(row) => crate::RepoOverview {
                repo: open.id,
                name: open.display_name.clone(),
                root: git_engine::slash_path(&open.root),
                ..row
            },
            Err(err) => {
                tracing::error!(error = ?err, context = "repository row in the agent");
                crate::RepoOverview {
                    repo: open.id,
                    name: open.display_name.clone(),
                    root: git_engine::slash_path(&open.root),
                    branch: None,
                    branch_at: None,
                    ahead: 0,
                    behind: 0,
                    dirty: false,
                    missing: true,
                    state: git_engine::RepoState::Clean,
                }
            }
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
        if let Some(agent) = agents.get(host).filter(|agent| !agent.is_gone()) {
            return Ok(agent.clone());
        }
        let sink = Arc::new(crate::agent_events::Sink {
            host: host.clone(),
            remotes: Arc::clone(&self.remotes),
            journal: Arc::clone(&self.journal),
            events: self.events.clone(),
            rows: Arc::clone(&self.cached_rows),
        });
        let events = self.events.clone();
        let label = host.label();
        let tell = move |state: BackendState| {
            let _ = events.send(crate::AppEvent::Backend {
                host: label.clone(),
                state,
            });
        };
        tell(BackendState::Starting);
        let lost = tell.clone();
        let (asked, passwords) = (host.clone(), Arc::clone(&self.passwords));
        let started = Agent::start(
            host,
            move |event| sink.take(event),
            move |question| crate::agent_events::answer_agent(&asked, &passwords, question),
            move |reason| lost(BackendState::Lost { reason }),
        );
        tell(if started.is_ok() {
            BackendState::Started
        } else {
            BackendState::Failed
        });
        let agent = Arc::new(started?);
        if let Host::Wsl { .. } = host {
            check_git(host, &agent)?;
        }
        if let Some(bytes) = self.graph_cache_budget()
            && let Err(err) = agent.ask::<()>(&Request::GraphBudget { bytes }, None)
        {
            tracing::error!(error = ?err, context = "graph budget in the agent");
        }
        agents.insert(host.clone(), agent.clone());
        Ok(agent)
    }
}
