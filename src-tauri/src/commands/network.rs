//! Remotes and the network: fetch, pull, push with progress, remote URLs and stored sign-ins (M1, F-082).

use super::blocking;
use app_state::{OperationKind, RepoId};
use git_engine::{GitError, NetworkStop};

#[tauri::command]
#[specta::specta]
pub async fn remotes(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<Vec<String>, GitError> {
    let app_state = state.state.clone();
    blocking("remotes", move || app_state.remotes(repo)).await
}

/// A network command's own lines go to the page as they arrive, and its phases into the
/// profile once it ends; every command that talks to a remote goes through here.
pub(super) fn with_progress<T>(
    operation: &'static str,
    remote: &str,
    channel: &tauri::ipc::Channel<String>,
    run: impl FnOnce(&mut dyn FnMut(&str)) -> Result<T, GitError>,
) -> Result<T, GitError> {
    let mut timer = git_engine::phases::PhaseTimer::new();
    let result = run(&mut |line: &str| {
        timer.observe(line);
        let _ = channel.send(line.to_owned());
    });
    crate::profile::network(operation, remote, timer, result.is_ok());
    result
}

/// `mutating` for a command that talks to a remote: `cancel_network` can stop it by the
/// id of its queue operation while it runs.
pub(super) async fn networking<T, F>(
    state: &std::sync::Arc<app_state::AppState>,
    repo: RepoId,
    kind: OperationKind,
    label: &'static str,
    work: F,
) -> Result<T, GitError>
where
    T: Send + 'static,
    F: FnOnce(NetworkStop) -> Result<T, GitError> + Send + 'static,
{
    networking_if(true, state, repo, kind, kind.title(), label, work).await
}

/// `networking` for a command that only sometimes reaches a remote: the footer offers Cancel
/// (`cancellable`) only when it does, since for the rest there is no git to stop.
pub(super) async fn networking_if<T, F>(
    cancellable: bool,
    state: &std::sync::Arc<app_state::AppState>,
    repo: RepoId,
    kind: OperationKind,
    title: &str,
    label: &'static str,
    work: F,
) -> Result<T, GitError>
where
    T: Send + 'static,
    F: FnOnce(NetworkStop) -> Result<T, GitError> + Send + 'static,
{
    let permit = if cancellable {
        state.enqueue_cancellable(repo, kind, title).await
    } else {
        state.enqueue(repo, kind, title).await
    };
    let run = cancellable.then(|| state.network_stop(permit.id()));
    let stop = run.as_ref().map(|run| run.token()).unwrap_or_default();
    let result = blocking(label, move || work(stop)).await;
    drop(run);
    permit.finish(result.is_ok());
    result
}

/// Stops the fetch, pull or push running as queue operation `operation`. `false` when
/// there is nothing to stop. Off the main thread: stopping waits for `taskkill`.
#[tauri::command]
#[specta::specta]
pub async fn cancel_network(
    state: tauri::State<'_, crate::AppContext>,
    operation: u32,
) -> Result<bool, GitError> {
    let app_state = state.state.clone();
    blocking("cancel_network", move || {
        Ok(app_state.cancel_network(operation))
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn fetch(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    remote: String,
    credential: Option<app_state::SignIn>,
    on_progress: tauri::ipc::Channel<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    networking(
        &state.state,
        repo,
        OperationKind::Fetch,
        "fetch",
        move |stop| {
            with_progress("fetch", &remote, &on_progress, |on_line| {
                app_state.fetch(repo, &remote, &stop, credential.as_ref(), on_line)
            })
        },
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn pull(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    remote: String,
    autostash: bool,
    credential: Option<app_state::SignIn>,
    on_progress: tauri::ipc::Channel<String>,
) -> Result<git_engine::PullOutcome, GitError> {
    let app_state = state.state.clone();
    let work = move |stop: NetworkStop| {
        with_progress("pull", &remote, &on_progress, |line| {
            app_state.pull(repo, &remote, autostash, &stop, credential.as_ref(), line)
        })
    };
    networking(&state.state, repo, OperationKind::Pull, "pull", work).await
}

#[tauri::command]
#[specta::specta]
pub async fn push(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    remote: String,
    force: bool,
    credential: Option<app_state::SignIn>,
    on_progress: tauri::ipc::Channel<String>,
) -> Result<Option<git_engine::Pushed>, GitError> {
    let app_state = state.state.clone();
    networking(
        &state.state,
        repo,
        OperationKind::Push,
        "push",
        move |stop| {
            with_progress("push", &remote, &on_progress, |on_line| {
                app_state.push(repo, &remote, force, &stop, credential.as_ref(), on_line)
            })
        },
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn remote_url(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    name: String,
) -> Result<Option<String>, GitError> {
    let app_state = state.state.clone();
    blocking("remote_url", move || app_state.remote_url(repo, &name)).await
}

fn store_error(err: app_state::SecretError) -> GitError {
    GitError::InvalidState(err.to_string())
}

/// Cogit's password store: locked or not, and the sign-ins in it; never a secret (R-891).
#[tauri::command]
#[specta::specta]
pub async fn credential_store(
    state: tauri::State<'_, crate::AppContext>,
) -> Result<app_state::CredentialStore, GitError> {
    let app_state = state.state.clone();
    blocking("credential_store", move || {
        app_state.credential_store().map_err(store_error)
    })
    .await
}

/// `false`: not the master password.
#[tauri::command]
#[specta::specta]
pub async fn unlock_credentials(
    state: tauri::State<'_, crate::AppContext>,
    password: String,
) -> Result<bool, GitError> {
    let app_state = state.state.clone();
    let password = zeroize::Zeroizing::new(password);
    blocking("unlock_credentials", move || {
        app_state.unlock_credentials(&password).map_err(store_error)
    })
    .await
}

/// The first store; `null`: no master password.
#[tauri::command]
#[specta::specta]
pub async fn create_credential_store(
    state: tauri::State<'_, crate::AppContext>,
    password: Option<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    let password = password.map(zeroize::Zeroizing::new);
    blocking("create_credential_store", move || {
        app_state
            .create_credential_store(password.as_deref().map(String::as_str))
            .map_err(store_error)
    })
    .await
}

/// `false`: `current` is not the master password. `next` `null`: none from now on.
#[tauri::command]
#[specta::specta]
pub async fn change_master_password(
    state: tauri::State<'_, crate::AppContext>,
    current: String,
    next: Option<String>,
) -> Result<bool, GitError> {
    let app_state = state.state.clone();
    let current = zeroize::Zeroizing::new(current);
    let next = next.map(zeroize::Zeroizing::new);
    blocking("change_master_password", move || {
        app_state
            .change_master_password(&current, next.as_deref().map(String::as_str))
            .map_err(store_error)
    })
    .await
}

/// A forgotten master password: a new one, and every stored sign-in is deleted.
#[tauri::command]
#[specta::specta]
pub async fn reset_credential_store(
    state: tauri::State<'_, crate::AppContext>,
    password: Option<String>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    let password = password.map(zeroize::Zeroizing::new);
    blocking("reset_credential_store", move || {
        app_state
            .reset_credential_store(password.as_deref().map(String::as_str))
            .map_err(store_error)
    })
    .await
}

/// The Sign in dialog's SSH key tab: `~/.ssh` and the private keys in it.
#[tauri::command]
#[specta::specta]
pub async fn ssh_keys() -> Result<app_state::SshKeys, GitError> {
    blocking("ssh_keys", || Ok(app_state::ssh_keys())).await
}

/// Preferences ▸ Authentication ▸ Add…: kept without asking the server.
#[tauri::command]
#[specta::specta]
pub async fn store_credential(
    state: tauri::State<'_, crate::AppContext>,
    credential: app_state::SignIn,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("store_credential", move || {
        app_state.store_credential(&credential).map_err(store_error)
    })
    .await
}

/// `ssh`: the host's ssh key, else its login.
#[tauri::command]
#[specta::specta]
pub async fn forget_credential(
    state: tauri::State<'_, crate::AppContext>,
    host: String,
    ssh: bool,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("forget_credential", move || {
        app_state.forget_credential(&host, ssh).map_err(store_error)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn forget_all_credentials(
    state: tauri::State<'_, crate::AppContext>,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    blocking("forget_all_credentials", move || {
        app_state.forget_all_credentials().map_err(store_error)
    })
    .await
}

/// Fetch Only of the Pull dialog; the answer names notes that diverged.
#[tauri::command]
#[specta::specta]
pub async fn fetch_with(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    remote: String,
    options: git_engine::FetchOptions,
    credential: Option<app_state::SignIn>,
    on_progress: tauri::ipc::Channel<String>,
) -> Result<git_engine::NotesFetch, GitError> {
    let app_state = state.state.clone();
    networking(
        &state.state,
        repo,
        OperationKind::Fetch,
        "fetch_with",
        move |stop| {
            with_progress("fetch", &remote, &on_progress, |on_line| {
                app_state.fetch_with(repo, &remote, options, &stop, credential.as_ref(), on_line)
            })
        },
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn pull_with(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    remote: String,
    options: git_engine::PullOptions,
    credential: Option<app_state::SignIn>,
    on_progress: tauri::ipc::Channel<String>,
) -> Result<git_engine::Pulled, GitError> {
    let app_state = state.state.clone();
    networking(
        &state.state,
        repo,
        OperationKind::Pull,
        "pull_with",
        move |stop| {
            with_progress("pull", &remote, &on_progress, |on_line| {
                app_state.pull_with(repo, &remote, options, &stop, credential.as_ref(), on_line)
            })
        },
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn push_with(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    options: git_engine::PushOptions,
    credential: Option<app_state::SignIn>,
    on_progress: tauri::ipc::Channel<String>,
) -> Result<git_engine::PushOutcome, GitError> {
    let app_state = state.state.clone();
    networking(
        &state.state,
        repo,
        OperationKind::Push,
        "push_with",
        move |stop| {
            let remote = options.remote.clone();
            with_progress("push", &remote, &on_progress, |on_line| {
                app_state.push_with(repo, &options, &stop, credential.as_ref(), on_line)
            })
        },
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn push_dry_run(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    options: git_engine::PushOptions,
    credential: Option<app_state::SignIn>,
    on_progress: tauri::ipc::Channel<String>,
) -> Result<git_engine::GitOutput, GitError> {
    let app_state = state.state.clone();
    // Not a Push: a system notice is named after the kind, and nothing is pushed.
    networking_if(
        true,
        &state.state,
        repo,
        OperationKind::Other,
        "Push dry run",
        "push_dry_run",
        move |stop| {
            let remote = options.remote.clone();
            with_progress("push", &remote, &on_progress, |on_line| {
                app_state.push_dry_run(repo, &options, &stop, credential.as_ref(), on_line)
            })
        },
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn push_notes(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    remote: String,
    credential: Option<app_state::SignIn>,
    on_progress: tauri::ipc::Channel<String>,
) -> Result<git_engine::PushOutcome, GitError> {
    let app_state = state.state.clone();
    networking(
        &state.state,
        repo,
        OperationKind::Push,
        "push_notes",
        move |stop| {
            with_progress("push", &remote, &on_progress, |on_line| {
                app_state.push_notes(repo, &remote, &stop, credential.as_ref(), on_line)
            })
        },
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn merge_notes(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    remote: String,
    namespace: String,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    super::mutating(
        &state.state,
        repo,
        OperationKind::Other,
        "merge_notes",
        move || app_state.merge_notes(repo, &remote, &namespace),
    )
    .await
}

/// The commits Push would send (the list capped at `limit`) and the notes the remote lacks.
#[tauri::command]
#[specta::specta]
pub async fn push_preview(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    local: String,
    remote: String,
    branch: String,
    limit: u32,
) -> Result<git_engine::PushPreview, GitError> {
    let app_state = state.state.clone();
    blocking("push_preview", move || {
        app_state.push_preview(repo, &local, &remote, &branch, limit as usize)
    })
    .await
}

/// Not queued, as `push_tags_preview`.
#[tauri::command]
#[specta::specta]
pub async fn push_atomic_support(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    options: git_engine::PushOptions,
) -> Result<Option<bool>, GitError> {
    let app_state = state.state.clone();
    blocking("push_atomic_support", move || {
        app_state.push_atomic_support(repo, &options)
    })
    .await
}

/// Not queued: it writes nothing, and its stall limits end a silent remote (R-746).
#[tauri::command]
#[specta::specta]
pub async fn push_tags_preview(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    options: git_engine::PushOptions,
) -> Result<Vec<String>, GitError> {
    let app_state = state.state.clone();
    blocking("push_tags_preview", move || {
        app_state.push_tags_preview(repo, &options)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn network_defaults(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
) -> Result<git_engine::NetworkDefaults, GitError> {
    let app_state = state.state.clone();
    blocking("network_defaults", move || app_state.network_defaults(repo)).await
}

#[tauri::command]
#[specta::specta]
pub async fn save_network_defaults(
    state: tauri::State<'_, crate::AppContext>,
    repo: RepoId,
    defaults: git_engine::NetworkDefaults,
) -> Result<(), GitError> {
    let app_state = state.state.clone();
    super::mutating(
        &state.state,
        repo,
        OperationKind::Other,
        "save_network_defaults",
        move || app_state.save_network_defaults(repo, &defaults),
    )
    .await
}
