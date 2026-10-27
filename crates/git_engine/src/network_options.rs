//! The Pull and Push dialogs (F-610): what they ask of git, the notes that travel beside
//! branches, and the choices a repository remembers.

use crate::{GitError, RepoHandle, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum PullMethod {
    Merge,
    /// As SmartGit: own commits go on top of the upstream's (R-841).
    #[default]
    Rebase,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum TagsMode {
    #[default]
    None,
    /// `--follow-tags`: annotated tags that point at pushed commits.
    Follow,
    All,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FetchOptions {
    /// `--tags --force`: new tags, and existing ones moved to where the remote has them.
    pub tags: bool,
    pub notes: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PullOptions {
    /// How a branch with commits of its own takes the upstream's; one without them only
    /// fast-forwards (R-841).
    pub method: PullMethod,
    pub fetch: FetchOptions,
    /// `--autostash`: the answer to `Pulled::stash_needed`.
    pub autostash: bool,
}

/// `stash_needed`: the pull only fetched, because the branch has to move and the working
/// tree has changes to tracked files; pull again with `autostash` once the user agrees.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Pulled {
    pub notes: NotesFetch,
    pub stash_needed: bool,
    /// How the branch took the upstream's commits; `None` for a fast-forward or a fetch only.
    pub integrated: Option<PullMethod>,
    /// Fetched, but the upstream is gone from the remote and nothing was merged; the
    /// journal's warning says so (R-840).
    pub upstream_gone: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PushOptions {
    pub remote: String,
    pub local: String,
    pub branch: String,
    pub set_upstream: bool,
    pub tags: TagsMode,
    pub notes: bool,
    pub force_with_lease: bool,
    /// With the lease: git reads the expected tip from the remote-tracking ref itself and
    /// checks it was integrated; an explicit tip would turn that check off (`apply_cas`).
    pub force_if_includes: bool,
    /// The remote-tracking tip the dialog showed, for a lease without the check; `None`: the
    /// branch must not exist there.
    pub lease_expect: Option<String>,
    pub no_verify: bool,
    pub atomic: bool,
}

/// What a repository remembers for the two dialogs. Never `force_with_lease`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NetworkDefaults {
    pub pull_method: PullMethod,
    pub pull_tags: bool,
    pub pull_notes: bool,
    pub push_tags: TagsMode,
    pub push_notes: bool,
    /// The remote the Push dialog opens on; `None`: where git would push.
    pub push_remote: Option<String>,
}

/// Namespaces fetched from a remote whose notes and the local ones both moved.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NotesFetch {
    pub remote: String,
    /// `refs/notes/<name>` names, as `git notes --ref` takes them without the prefix.
    pub diverged: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PushOutcome {
    /// Git's own words when the remote refused the notes as non-fast-forward; the branch
    /// itself went through.
    pub notes_rejected: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PushCommit {
    pub oid: String,
    pub summary: String,
    /// The author through `.mailmap`, as `git log` dates a commit.
    pub author: crate::Signature,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PushPreview {
    pub total: u32,
    pub commits: Vec<PushCommit>,
    /// Commits of the remote-tracking branch the local one lacks; 0 without it.
    pub behind: u32,
    pub local_tip: Option<String>,
    /// `refs/remotes/<remote>/<branch>` as last fetched; `None`: no such branch there.
    pub remote_tip: Option<String>,
    /// Unix ms of the newest sign of a fetch from the remote (`fetched_at`).
    #[specta(type = Option<specta_typescript::Number>)]
    pub fetched_at: Option<u64>,
    /// Local notes the remote lacks, from the notes last fetched from it; `None` while
    /// nothing was fetched to compare with.
    pub notes_unpushed: Option<u32>,
    pub has_local_notes: bool,
}

const LOCAL_NOTES: &str = "refs/notes/";

fn notes_mirror(remote: &str) -> String {
    format!("refs/notes-remote/{remote}/")
}

/// A row of a list of commits to push; what cannot be read stays empty.
fn push_commit(repo: &gix::Repository, id: gix::ObjectId, mailmap: &crate::Mailmap) -> PushCommit {
    let commit = repo.find_commit(id).ok();
    let summary = commit
        .as_ref()
        .and_then(|commit| commit.message().ok().map(|m| m.summary().to_string()))
        .unwrap_or_default();
    let author = commit
        .as_ref()
        .and_then(|commit| commit.author().ok())
        .map(|sig| crate::commit::signature(sig, mailmap))
        .unwrap_or_default();
    PushCommit {
        oid: id.to_string(),
        summary,
        author,
    }
}

const NOTES_REFSPEC: &str = "refs/notes/*:refs/notes/*";

/// The dialog's push with `--dry-run`: nothing moves, and the output window shows one record.
fn push_dry_run_args(options: &PushOptions) -> Vec<String> {
    let mut args = push_args(options);
    args.insert(1, "--dry-run".into());
    args
}

/// The dialog's push asked only which refs it would send: nothing goes, no hook runs.
fn push_tags_args(options: &PushOptions) -> Vec<String> {
    let tags = if options.tags == TagsMode::All {
        "--tags"
    } else {
        "--follow-tags"
    };
    let mut args: Vec<String> = ["push", "--dry-run", "--porcelain", "--no-verify", tags]
        .map(String::from)
        .to_vec();
    args.push(options.remote.clone());
    args.push(format!(
        "refs/heads/{}:refs/heads/{}",
        options.local, options.branch
    ));
    args
}

/// The dialog's push asked only whether the remote takes `--atomic`. git checks that right
/// after the remote names its refs, but over HTTP only when a ref has to go: the probe names
/// one the push would create. A dry run: nothing is created.
fn push_atomic_args(options: &PushOptions) -> Vec<String> {
    let mut args: Vec<String> = [
        "push",
        "--dry-run",
        "--porcelain",
        "--no-verify",
        "--atomic",
    ]
    .map(String::from)
    .to_vec();
    args.push(options.remote.clone());
    args.push(format!(
        "refs/heads/{}:refs/cogit/atomic-probe",
        options.local
    ));
    args
}

/// `Some(false)`: git or its remote helper said the remote does not take it; `Some(true)`: git went past that check
/// to the refs (`To <url>`); `None`: it stopped before hearing from the remote.
fn atomic_answer(stdout: &str, stderr: &str) -> Option<bool> {
    if stderr.contains("does not support --atomic") {
        return Some(false);
    }
    stdout
        .lines()
        .any(|line| line.starts_with("To "))
        .then_some(true)
}

/// Tags in `git push --porcelain` output (`<flag>\t<from>:<to>\t<summary>`) that would be
/// sent: not those already there (`=`) or refused (`!`). By name, sorted.
fn tags_in_porcelain(stdout: &str) -> Vec<String> {
    let mut tags: Vec<String> = stdout
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let flag = fields.next()?;
            let (_, to) = fields.next()?.split_once(':')?;
            let name = to.strip_prefix("refs/tags/")?;
            (flag != "=" && flag != "!").then(|| name.to_owned())
        })
        .collect();
    tags.sort();
    tags.dedup();
    tags
}

/// `url` as `git fetch` writes it into FETCH_HEAD: no user or password, no trailing `/` or
/// `.git` (`transport_anonymize_url`, `store_updated_refs`).
fn fetch_head_url(url: &str) -> String {
    let anonymous = match (url.find("://"), url.find('@')) {
        (Some(scheme), Some(at)) if at > scheme + 3 && !url[scheme + 3..at].contains('/') => {
            format!("{}{}", &url[..scheme + 3], &url[at + 1..])
        }
        (None, Some(at)) if url[at + 1..].contains(':') => url[at + 1..].to_owned(),
        _ => url.to_owned(),
    };
    let trimmed = anonymous.trim_end_matches('/');
    trimmed.strip_suffix(".git").unwrap_or(trimmed).to_owned()
}

#[must_use]
pub fn fetch_args(remote: &str, tags: bool) -> Vec<&str> {
    let mut args = vec!["fetch", "--progress", "--prune"];
    if tags {
        args.extend(["--tags", "--force"]);
    }
    args.push(remote);
    args
}

/// `fast_forward`: the branch has no commits of its own. A merge says `--no-rebase`:
/// without it `pull.rebase` rebases, and with no `pull.*` set git refuses diverged branches.
#[must_use]
pub fn pull_args<'a>(remote: &'a str, options: &PullOptions, fast_forward: bool) -> Vec<&'a str> {
    let mut args = vec!["pull", "--progress", "--prune"];
    if options.fetch.tags {
        args.extend(["--tags", "--force"]);
    }
    args.push(match (fast_forward, options.method) {
        (true, _) => "--ff-only",
        (false, PullMethod::Rebase) => "--rebase",
        (false, PullMethod::Merge) => "--no-rebase",
    });
    if options.autostash {
        args.push("--autostash");
    }
    args.push(remote);
    args
}

#[must_use]
pub fn push_args(options: &PushOptions) -> Vec<String> {
    let mut args: Vec<String> = ["push", "--progress"].map(String::from).to_vec();
    if options.set_upstream {
        args.push("--set-upstream".into());
    }
    match options.tags {
        TagsMode::None => {}
        TagsMode::Follow => args.push("--follow-tags".into()),
        TagsMode::All => args.push("--tags".into()),
    }
    if options.no_verify {
        args.push("--no-verify".into());
    }
    if options.atomic {
        args.push("--atomic".into());
    }
    if options.force_with_lease {
        args.extend(lease_args(options));
    }
    args.push(options.remote.clone());
    args.push(format!(
        "refs/heads/{}:refs/heads/{}",
        options.local, options.branch
    ));
    if options.notes {
        args.push(NOTES_REFSPEC.into());
    }
    args
}

/// The lease of one branch, never of every ref the push names.
fn lease_args(options: &PushOptions) -> Vec<String> {
    let name = format!("refs/heads/{}", options.branch);
    if options.force_if_includes {
        // As the toolbar's forced push does: a lease alone passes when a background fetch
        // moved the tracking ref.
        return vec![
            format!("--force-with-lease={name}"),
            "--force-if-includes".into(),
        ];
    }
    let expect = options.lease_expect.as_deref().unwrap_or_default();
    vec![format!("--force-with-lease={name}:{expect}")]
}

impl RepoHandle {
    pub fn fetch_options(
        &self,
        remote: &str,
        options: FetchOptions,
        credential: impl Fn(&str) -> Option<crate::Credential>,
        mut on_line: impl FnMut(&str),
    ) -> Result<NotesFetch> {
        let notes = if options.notes {
            self.fetch_notes(remote, &credential, &mut on_line)?
        } else {
            NotesFetch::default()
        };
        let auth = self.auth_for_fetch(remote, &credential);
        self.stream(&auth, &fetch_args(remote, options.tags), &mut on_line)?;
        Ok(notes)
    }

    /// Pull; notes come first, since they touch no working tree. git refuses `pull
    /// --rebase` over changes to tracked files before it even fetches, whether or not the
    /// branch would move: with such changes this fetches only, and asks for a stash only
    /// when the upstream has commits HEAD lacks (unknown counts as having them).
    pub fn pull_options(
        &self,
        remote: &str,
        options: PullOptions,
        credential: impl Fn(&str) -> Option<crate::Credential>,
        mut on_line: impl FnMut(&str),
    ) -> Result<Pulled> {
        let notes = if options.fetch.notes {
            self.fetch_notes(remote, &credential, &mut on_line)?
        } else {
            NotesFetch::default()
        };
        let auth = self.auth_for_fetch(remote, &credential);
        let mut pulled = Pulled {
            notes,
            ..Pulled::default()
        };
        if !options.autostash && self.has_tracked_changes()? {
            self.stream(&auth, &fetch_args(remote, options.fetch.tags), &mut on_line)?;
            pulled.stash_needed = self.head_divergence().map(|(_, behind)| behind) != Some(0);
            return Ok(pulled);
        }
        // Judged before the fetch: commits of its own stay its own whatever arrives.
        let fast_forward = self.head_divergence().map(|(ahead, _)| ahead) == Some(0);
        let args = pull_args(remote, &options, fast_forward);
        match self.stream(&auth, &args, &mut on_line) {
            Ok(()) => {
                pulled.integrated = (!fast_forward).then_some(options.method);
                Ok(pulled)
            }
            Err(GitError::Command(failure))
                if self
                    .upstream_gone_after(&failure.command, &failure.stderr)
                    .is_some() =>
            {
                pulled.upstream_gone = true;
                Ok(pulled)
            }
            Err(err) => Err(err),
        }
    }

    /// Commits HEAD has that its upstream lacks, and the other way round; `None` when that
    /// cannot be told.
    fn head_divergence(&self) -> Option<(u32, u32)> {
        let name = self.repo.head_name().ok()??;
        let tracking = self
            .repo
            .branch_remote_tracking_ref_name(name.as_ref(), gix::remote::Direction::Fetch)?
            .ok()?;
        let upstream = self
            .repo
            .find_reference(tracking.as_ref())
            .ok()?
            .peel_to_id()
            .ok()?;
        let head = self.repo.head_id().ok()?;
        self.count_divergence(head.detach(), upstream.detach())
    }

    /// One `git push` for the branch, its tags and notes, so `--atomic`, `--dry-run` and the
    /// hook cover them all; notes refused as behind are an outcome, not a failure.
    pub fn push_options(
        &self,
        options: &PushOptions,
        credential: impl Fn(&str) -> Option<crate::Credential>,
        mut on_line: impl FnMut(&str),
    ) -> Result<PushOutcome> {
        let auth = self.auth_for_push(&options.remote, &credential);
        let args = push_args(options);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        match self.stream(&auth, &args, &mut on_line) {
            Ok(()) if options.notes => self.mirror_notes(&options.remote),
            Ok(()) => Ok(PushOutcome {
                notes_rejected: None,
            }),
            Err(GitError::Command(failure))
                if options.notes && only_notes_refused(&output_text(&failure)) =>
            {
                Ok(PushOutcome {
                    notes_rejected: Some(output_text(&failure)),
                })
            }
            Err(err) => Err(err),
        }
    }

    /// What the dialog's push would do, and its record for the output window.
    pub fn push_dry_run(
        &self,
        options: &PushOptions,
        credential: impl Fn(&str) -> Option<crate::Credential>,
        mut on_line: impl FnMut(&str),
    ) -> Result<crate::GitOutput> {
        let auth = self.auth_for_push(&options.remote, &credential);
        let args = push_dry_run_args(options);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        self.run_streaming_recorded(&args, &auth, &mut on_line)
    }

    /// `refs/notes/*` to the same names. Never forced: a remote that moved its notes
    /// refuses, and the dialog offers Fetch notes and merge.
    pub fn push_notes(
        &self,
        remote: &str,
        credential: impl Fn(&str) -> Option<crate::Credential>,
        mut on_line: impl FnMut(&str),
    ) -> Result<PushOutcome> {
        if self.refs_under(LOCAL_NOTES).is_empty() {
            return Ok(PushOutcome {
                notes_rejected: None,
            });
        }
        let auth = self.auth_for_push(remote, &credential);
        let args = ["push", "--progress", remote, NOTES_REFSPEC];
        match self.stream(&auth, &args, &mut on_line) {
            Ok(()) => self.mirror_notes(remote),
            Err(GitError::Command(failure)) if only_notes_refused(&output_text(&failure)) => {
                Ok(PushOutcome {
                    notes_rejected: Some(output_text(&failure)),
                })
            }
            Err(err) => Err(err),
        }
    }

    /// The local notes as what the remote has, now that they went there.
    fn mirror_notes(&self, remote: &str) -> Result<PushOutcome> {
        for (name, id) in self.refs_under(LOCAL_NOTES) {
            let mirror = format!("{}{}", notes_mirror(remote), &name[LOCAL_NOTES.len()..]);
            self.run_git(&["update-ref", &mirror, &id])?;
        }
        Ok(PushOutcome {
            notes_rejected: None,
        })
    }

    /// Fetches the remote's notes beside ours (`refs/notes-remote/<remote>/*`), moves
    /// every namespace that only fell behind, and names those that diverged: they are
    /// never overwritten.
    pub fn fetch_notes(
        &self,
        remote: &str,
        credential: &impl Fn(&str) -> Option<crate::Credential>,
        on_line: &mut impl FnMut(&str),
    ) -> Result<NotesFetch> {
        let auth = self.auth_for_fetch(remote, credential);
        let mirror = notes_mirror(remote);
        let spec = format!("+{LOCAL_NOTES}*:{mirror}*");
        self.stream(&auth, &["fetch", "--progress", remote, &spec], on_line)?;

        let mut diverged = Vec::new();
        for (name, theirs) in self.refs_under(&mirror) {
            let namespace = &name[mirror.len()..];
            let ours_name = format!("{LOCAL_NOTES}{namespace}");
            let Some((_, ours)) = self
                .refs_under(&ours_name)
                .into_iter()
                .find(|(name, _)| *name == ours_name)
            else {
                self.run_git(&["update-ref", &ours_name, &theirs])?;
                continue;
            };
            if ours == theirs || self.is_ancestor(&theirs, &ours)? {
                continue;
            }
            if self.is_ancestor(&ours, &theirs)? {
                self.run_git(&["update-ref", &ours_name, &theirs, &ours])?;
            } else {
                diverged.push(namespace.to_owned());
            }
        }
        diverged.sort();
        Ok(NotesFetch {
            remote: remote.to_owned(),
            diverged,
        })
    }

    /// `git notes merge` of what `fetch_notes` left beside ours; a conflict is aborted so
    /// that no half-merged notes worktree stays, and git's words go on as the error.
    pub fn merge_notes(&self, remote: &str, namespace: &str) -> Result<()> {
        let theirs = format!("{}{namespace}", notes_mirror(remote));
        let ours = format!("{LOCAL_NOTES}{namespace}");
        match self.run_git(&["notes", "--ref", &ours, "merge", &theirs]) {
            Ok(_) => Ok(()),
            Err(err) => {
                if let Err(abort) = self.run_git(&["notes", "--ref", &ours, "merge", "--abort"]) {
                    tracing::error!(error = ?abort, context = "aborting a notes merge");
                }
                Err(err)
            }
        }
    }

    /// Commits `HEAD`'s `local` branch would add to `remote`: those not reachable from
    /// the remote branch, or, while it is unknown, from any branch of that remote; and how
    /// the two stand.
    #[must_use]
    pub fn push_preview(
        &self,
        local: &str,
        remote: &str,
        branch: &str,
        limit: usize,
    ) -> PushPreview {
        let (total, commits) = self.unpushed_commits(local, remote, branch, limit);
        let local_tip = self.resolve_commit(&format!("refs/heads/{local}")).ok();
        let remote_tip = self
            .resolve_commit(&format!("refs/remotes/{remote}/{branch}"))
            .ok();
        let behind = local_tip
            .zip(remote_tip)
            .and_then(|(ours, theirs)| self.count_divergence(ours, theirs))
            .map_or(0, |(_, behind)| behind);
        let mirror = notes_mirror(remote);
        let ours = self.refs_under(LOCAL_NOTES);
        let theirs = self.refs_under(&mirror);
        PushPreview {
            total,
            commits,
            behind,
            local_tip: local_tip.map(|id| id.to_string()),
            remote_tip: remote_tip.map(|id| id.to_string()),
            fetched_at: self.fetched_at(remote, branch),
            has_local_notes: !ours.is_empty(),
            notes_unpushed: (!theirs.is_empty() && !ours.is_empty())
                .then(|| self.count_notes_unpushed(&ours, &theirs, &mirror)),
        }
    }

    /// Tags `options` would send with its tag mode, asked of the remote as a background check
    /// asks: no prompt anywhere. A branch the remote would refuse still has its tags listed,
    /// so a failure that printed refs is read too.
    pub fn push_tags_preview(
        &self,
        options: &PushOptions,
        credential: impl FnOnce(&str) -> Option<crate::Credential>,
    ) -> Result<Vec<String>> {
        if options.tags == TagsMode::None {
            return Ok(Vec::new());
        }
        let auth = self.auth_env(&options.remote, gix::remote::Direction::Push, credential);
        let refusal = crate::pulse::askpass_refusal(self.has_own_ssh_command());
        let mut env = crate::pulse::quiet_env(&refusal);
        env.extend(
            auth.iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        );
        let asked = push_tags_args(options);
        let mut args: Vec<&str> = self.background_options();
        args.extend(asked.iter().map(String::as_str));
        match self.read_git_with(&args, &env) {
            Ok(stdout) => Ok(tags_in_porcelain(&stdout)),
            Err(GitError::Command(failure)) if failure.stdout.contains('\t') => {
                Ok(tags_in_porcelain(&failure.stdout))
            }
            Err(err) => Err(err),
        }
    }

    /// Whether the remote takes `--atomic` (`atomic_answer`), asked as quietly as the tags are.
    pub fn push_atomic_support(
        &self,
        options: &PushOptions,
        credential: impl FnOnce(&str) -> Option<crate::Credential>,
    ) -> Result<Option<bool>> {
        let auth = self.auth_env(&options.remote, gix::remote::Direction::Push, credential);
        let refusal = crate::pulse::askpass_refusal(self.has_own_ssh_command());
        let mut env = crate::pulse::quiet_env(&refusal);
        env.extend(
            auth.iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        );
        let asked = push_atomic_args(options);
        let mut args: Vec<&str> = self.background_options();
        args.extend(asked.iter().map(String::as_str));
        match self.read_git_with(&args, &env) {
            Ok(_) => Ok(Some(true)),
            Err(GitError::Command(failure)) => Ok(atomic_answer(&failure.stdout, &failure.stderr)),
            Err(err) => Err(err),
        }
    }

    /// Git keeps no record of a fetch: FETCH_HEAD's time while its lines name the remote's
    /// URL, else the last move of the remote-tracking branch (R-746). Unix ms.
    fn fetched_at(&self, remote: &str, branch: &str) -> Option<u64> {
        let moved = self.reflog_newest_ms(&format!("refs/remotes/{remote}/{branch}"));
        self.fetch_head_ms(remote).max(moved)
    }

    fn fetch_head_ms(&self, remote: &str) -> Option<u64> {
        let path = self.git_dir().join("FETCH_HEAD");
        let text = std::fs::read_to_string(&path).ok()?;
        let written = self
            .repo
            .config_snapshot()
            .plumbing()
            .string_by("remote", Some(remote.into()), "url")
            .map(|value| value.to_string());
        let urls: Vec<String> = [written, self.url_of(remote, gix::remote::Direction::Fetch)]
            .into_iter()
            .flatten()
            .map(|url| fetch_head_url(&url))
            .collect();
        let named = text.lines().any(|line| {
            line.rsplit_once(" of ")
                .is_some_and(|(_, url)| urls.iter().any(|known| known == url.trim_end()))
        });
        if !named {
            return None;
        }
        let modified = std::fs::metadata(&path).ok()?.modified().ok()?;
        let since = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
        u64::try_from(since.as_millis()).ok()
    }

    fn reflog_newest_ms(&self, name: &str) -> Option<u64> {
        let reference = self.repo.find_reference(name).ok()?;
        let mut platform = reference.log_iter();
        let newest = platform.rev().ok()??.next()?.ok()?;
        u64::try_from(newest.signature.time.seconds)
            .ok()?
            .checked_mul(1000)
    }

    fn unpushed_commits(
        &self,
        local: &str,
        remote: &str,
        branch: &str,
        limit: usize,
    ) -> (u32, Vec<PushCommit>) {
        let Ok(tip) = self.resolve_commit(&format!("refs/heads/{local}")) else {
            return (0, Vec::new());
        };
        let exact = format!("refs/remotes/{remote}/{branch}");
        let mut hidden: Vec<gix::ObjectId> = self
            .resolve_commit(&exact)
            .map(|id| vec![id])
            .unwrap_or_default();
        if hidden.is_empty() {
            hidden = self
                .refs_under(&format!("refs/remotes/{remote}/"))
                .iter()
                .filter_map(|(_, id)| self.resolve_commit(id).ok())
                .collect();
        }
        self.push_commits(&self.only_reachable(Some(tip), hidden), limit)
    }

    /// The count and the first `limit` rows of a list of commits to push.
    pub(crate) fn push_commits(
        &self,
        ids: &[gix::ObjectId],
        limit: usize,
    ) -> (u32, Vec<PushCommit>) {
        let mailmap = self.mailmap();
        let commits = ids
            .iter()
            .take(limit)
            .map(|id| push_commit(&self.repo, *id, &mailmap))
            .collect();
        (u32::try_from(ids.len()).unwrap_or(u32::MAX), commits)
    }

    fn count_notes_unpushed(
        &self,
        ours: &[(String, String)],
        theirs: &[(String, String)],
        mirror: &str,
    ) -> u32 {
        let mut count = 0_u32;
        for (name, id) in ours {
            let namespace = &name[LOCAL_NOTES.len()..];
            let mine = self.noted_blobs_at(id);
            let there = theirs
                .iter()
                .find(|(theirs, _)| theirs.strip_prefix(mirror) == Some(namespace))
                .map(|(_, id)| self.noted_blobs_at(id))
                .unwrap_or_default();
            let differing = mine
                .iter()
                .filter(|(commit, blob)| there.get(*commit) != Some(*blob))
                .count();
            count = count.saturating_add(u32::try_from(differing).unwrap_or(u32::MAX));
        }
        count
    }

    fn noted_blobs_at(
        &self,
        notes_commit: &str,
    ) -> std::collections::HashMap<String, gix::ObjectId> {
        let mut found = std::collections::HashMap::new();
        let full = self.repo.object_hash().len_in_hex();
        let tree = self
            .resolve_commit(notes_commit)
            .ok()
            .and_then(|id| self.repo.find_commit(id).ok())
            .and_then(|commit| commit.tree().ok());
        if let Some(tree) = tree {
            self.collect_noted(tree, "", full, &mut |name, blob| {
                found.insert(name, blob);
            });
        }
        found
    }

    /// (full name, target hex) of the refs under `prefix`, sorted. A prefix that is a whole
    /// ref name finds that ref.
    fn refs_under(&self, prefix: &str) -> Vec<(String, String)> {
        let mut refs = self.ref_tips(&[prefix]);
        refs.retain(|(_, target)| !target.is_empty());
        refs
    }

    fn auth_for_fetch(
        &self,
        remote: &str,
        credential: &impl Fn(&str) -> Option<crate::Credential>,
    ) -> Vec<(String, String)> {
        self.auth_env(remote, gix::remote::Direction::Fetch, credential)
    }

    fn auth_for_push(
        &self,
        remote: &str,
        credential: &impl Fn(&str) -> Option<crate::Credential>,
    ) -> Vec<(String, String)> {
        self.auth_env(remote, gix::remote::Direction::Push, credential)
    }

    fn stream(
        &self,
        auth: &[(String, String)],
        args: &[&str],
        on_line: &mut impl FnMut(&str),
    ) -> Result<()> {
        self.run_streaming(args, auth, |line| on_line(line))
    }

    /// What this repository remembers for the dialogs, from its own config only. Read
    /// without a process: a git run would journal the whole config, secrets included.
    pub fn network_defaults(&self) -> Result<NetworkDefaults> {
        let mut defaults = NetworkDefaults::default();
        if let Some(value) = self.local_choice("cogit.pullMethod") {
            defaults.pull_method = if value == "merge" {
                PullMethod::Merge
            } else {
                PullMethod::Rebase
            };
        }
        if let Some(value) = self.local_choice("cogit.pullTags") {
            defaults.pull_tags = value == "true";
        }
        if let Some(value) = self.local_choice("cogit.pullNotes") {
            defaults.pull_notes = value == "true";
        }
        if let Some(value) = self.local_choice("cogit.pushTags") {
            defaults.push_tags = match value.as_str() {
                "follow" => TagsMode::Follow,
                "all" => TagsMode::All,
                _ => TagsMode::None,
            };
        }
        if let Some(value) = self.local_choice("cogit.pushNotes") {
            defaults.push_notes = value == "true";
        }
        defaults.push_remote = self.local_choice("cogit.pushRemote");
        Ok(defaults)
    }

    fn local_choice(&self, key: &str) -> Option<String> {
        let snapshot = self.repo.config_snapshot();
        snapshot
            .plumbing()
            .string_filter(key, |meta| {
                matches!(
                    meta.source,
                    gix::config::Source::Local | gix::config::Source::Worktree
                )
            })
            .map(|value| value.to_string())
    }

    /// Absent means automatic; cleared only when it is there.
    fn forget_choice(&self, key: &str) -> Result<()> {
        if self.local_choice(key).is_some() {
            self.run_git(&["config", "--local", "--unset-all", key])?;
        }
        Ok(())
    }

    pub fn save_network_defaults(&self, defaults: &NetworkDefaults) -> Result<()> {
        let flag = |on: bool| if on { "true" } else { "false" };
        let method = match defaults.pull_method {
            PullMethod::Merge => "merge",
            PullMethod::Rebase => "rebase",
        };
        let tags = match defaults.push_tags {
            TagsMode::None => "none",
            TagsMode::Follow => "follow",
            TagsMode::All => "all",
        };
        let mut pairs = vec![
            ("cogit.pullMethod", method),
            ("cogit.pullTags", flag(defaults.pull_tags)),
            ("cogit.pullNotes", flag(defaults.pull_notes)),
            ("cogit.pushTags", tags),
            ("cogit.pushNotes", flag(defaults.push_notes)),
        ];
        match &defaults.push_remote {
            Some(remote) => pairs.push(("cogit.pushRemote", remote.as_str())),
            None => self.forget_choice("cogit.pushRemote")?,
        }
        for (key, value) in pairs {
            self.run_git(&["config", "--local", "--replace-all", "--", key, value])?;
        }
        Ok(())
    }
}

fn output_text(failure: &crate::GitCommandError) -> String {
    format!("{}{}", failure.stdout, failure.stderr)
}

/// Every ref git refused is notes it refused as behind (` ! [rejected] … -> refs/notes/…`),
/// not refused by a hook or the remote: those refuse it again whatever the dialog does.
fn only_notes_refused(output: &str) -> bool {
    let refused: Vec<&str> = output
        .lines()
        .filter(|line| line.starts_with(" ! ["))
        .collect();
    !refused.is_empty()
        && refused
            .iter()
            .all(|line| line.starts_with(" ! [rejected]") && line.contains(" -> refs/notes/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push(f: impl FnOnce(&mut PushOptions)) -> Vec<String> {
        let mut options = PushOptions {
            remote: "origin".into(),
            local: "main".into(),
            branch: "main".into(),
            ..PushOptions::default()
        };
        f(&mut options);
        push_args(&options)
    }

    #[test]
    fn a_plain_push_names_the_branches_and_nothing_else() {
        assert_eq!(
            push(|_| {}),
            [
                "push",
                "--progress",
                "origin",
                "refs/heads/main:refs/heads/main"
            ]
        );
    }

    #[test]
    fn options_become_flags_before_the_remote() {
        let args = push(|o| {
            o.set_upstream = true;
            o.tags = TagsMode::Follow;
            o.force_with_lease = true;
            o.force_if_includes = true;
        });
        assert_eq!(
            args,
            [
                "push",
                "--progress",
                "--set-upstream",
                "--follow-tags",
                "--force-with-lease=refs/heads/main",
                "--force-if-includes",
                "origin",
                "refs/heads/main:refs/heads/main"
            ]
        );
        assert!(push(|o| o.tags = TagsMode::All).contains(&"--tags".to_owned()));
    }

    #[test]
    fn a_lease_without_the_includes_check_names_the_tip_it_expects() {
        let named = push(|o| {
            o.force_with_lease = true;
            o.lease_expect = Some("abc".into());
        });
        assert!(
            named.contains(&"--force-with-lease=refs/heads/main:abc".to_owned()),
            "{named:?}"
        );
        assert!(!named.contains(&"--force-if-includes".to_owned()));
        let absent = push(|o| o.force_with_lease = true);
        assert!(
            absent.contains(&"--force-with-lease=refs/heads/main:".to_owned()),
            "{absent:?}"
        );
    }

    #[test]
    fn pull_picks_one_way_to_integrate() {
        let mut options = PullOptions::default();
        assert!(pull_args("o", &options, false).contains(&"--rebase"));
        assert!(pull_args("o", &options, true).contains(&"--ff-only"));
        options.method = PullMethod::Merge;
        assert!(pull_args("o", &options, false).contains(&"--no-rebase"));
        let args = pull_args("o", &options, true);
        assert!(args.contains(&"--ff-only") && !args.contains(&"--no-rebase"));
    }

    #[test]
    fn tags_are_fetched_and_forced_together() {
        assert!(
            fetch_args("o", true)
                .windows(2)
                .any(|w| w == ["--tags", "--force"])
        );
        assert!(!fetch_args("o", false).contains(&"--tags"));
        let options = PullOptions {
            fetch: FetchOptions {
                tags: true,
                notes: false,
            },
            ..PullOptions::default()
        };
        assert!(pull_args("o", &options, false).contains(&"--force"));
    }

    #[test]
    fn hook_and_atomic_flags_come_before_the_remote() {
        let args = push(|o| {
            o.no_verify = true;
            o.atomic = true;
        });
        assert_eq!(
            args,
            [
                "push",
                "--progress",
                "--no-verify",
                "--atomic",
                "origin",
                "refs/heads/main:refs/heads/main"
            ]
        );
    }

    // Over HTTP git checks `atomic` only when a ref has to go: an up-to-date branch passed.
    #[test]
    fn the_atomic_probe_asks_with_a_ref_the_push_would_create() {
        let options = PushOptions {
            remote: "origin".into(),
            local: "main".into(),
            branch: "main".into(),
            ..PushOptions::default()
        };
        assert_eq!(
            push_atomic_args(&options),
            [
                "push",
                "--dry-run",
                "--porcelain",
                "--no-verify",
                "--atomic",
                "origin",
                "refs/heads/main:refs/cogit/atomic-probe"
            ]
        );
    }

    #[test]
    fn a_remote_helper_without_atomic_is_a_refusal_too() {
        let helper = "fatal: helper https does not support --atomic\n";
        assert_eq!(atomic_answer("", helper), Some(false));
    }

    #[test]
    fn an_atomic_push_is_known_refused_only_by_the_words_of_git_and_known_taken_past_them() {
        let refused = "fatal: the receiving end does not support --atomic push\n";
        assert_eq!(atomic_answer("", refused), Some(false));
        let reached =
            "To ../r.git\n!\trefs/heads/main:refs/heads/main\t[rejected] (fetch first)\nDone\n";
        assert_eq!(
            atomic_answer(reached, "error: failed to push some refs\n"),
            Some(true)
        );
        let unreachable = "fatal: unable to access 'https://x/': timed out\n";
        assert_eq!(atomic_answer("", unreachable), None);
    }

    #[test]
    fn the_notes_go_in_the_same_push_as_the_branch_never_forced() {
        assert_eq!(
            push(|o| {
                o.notes = true;
                o.atomic = true;
            }),
            [
                "push",
                "--progress",
                "--atomic",
                "origin",
                "refs/heads/main:refs/heads/main",
                "refs/notes/*:refs/notes/*"
            ]
        );
    }

    #[test]
    fn only_notes_refused_as_behind_leave_the_rest_of_the_push_standing() {
        let notes = " ! [rejected]        refs/notes/commits -> refs/notes/commits (fetch first)\n";
        let branch = " ! [rejected]        main -> main (non-fast-forward)\n";
        let hook = " ! [remote rejected] refs/notes/commits -> refs/notes/commits (pre-receive hook declined)\n";
        let pushed = "   1a2b3c4..5d6e7f8  main -> main\n";
        assert!(only_notes_refused(&format!(
            "To ../r.git\n{pushed}{notes}error: failed to push some refs\n"
        )));
        assert!(!only_notes_refused(&format!("{notes}{branch}")));
        assert!(!only_notes_refused(hook));
        assert!(!only_notes_refused(
            "fatal: unable to access 'https://x/'\n"
        ));
    }

    #[test]
    fn a_dry_run_is_the_same_push_told_to_send_nothing_with_the_notes_beside() {
        let args = push_dry_run_args(&PushOptions {
            remote: "origin".into(),
            local: "main".into(),
            branch: "main".into(),
            notes: true,
            ..PushOptions::default()
        });
        assert_eq!(
            args,
            [
                "push",
                "--dry-run",
                "--progress",
                "origin",
                "refs/heads/main:refs/heads/main",
                "refs/notes/*:refs/notes/*"
            ]
        );
    }

    #[test]
    fn the_tag_preview_asks_without_the_hook_or_sending() {
        let options = PushOptions {
            remote: "origin".into(),
            local: "main".into(),
            branch: "dev".into(),
            tags: TagsMode::All,
            ..PushOptions::default()
        };
        assert_eq!(
            push_tags_args(&options),
            [
                "push",
                "--dry-run",
                "--porcelain",
                "--no-verify",
                "--tags",
                "origin",
                "refs/heads/main:refs/heads/dev"
            ]
        );
    }

    #[test]
    fn porcelain_names_the_tags_that_would_go() {
        let out = "To C:/r.git\n\
                   !\trefs/heads/main:refs/heads/main\t[rejected] (non-fast-forward)\n\
                   *\trefs/tags/v2:refs/tags/v2\t[new tag]\n\
                   =\trefs/tags/v1:refs/tags/v1\t[up to date]\n\
                   *\trefs/tags/v0:refs/tags/v0\t[new tag]\n\
                   Done\n";
        assert_eq!(tags_in_porcelain(out), ["v0", "v2"]);
    }

    #[test]
    fn fetch_head_names_a_remote_without_its_user_or_dot_git() {
        assert_eq!(
            fetch_head_url("https://github.com/a/b.git"),
            "https://github.com/a/b"
        );
        assert_eq!(
            fetch_head_url("https://me:secret@host.com/a/b/"),
            "https://host.com/a/b"
        );
        assert_eq!(fetch_head_url("git@github.com:a/b.git"), "github.com:a/b");
        assert_eq!(fetch_head_url(r"C:\work\origin.git"), r"C:\work\origin");
        assert_eq!(
            fetch_head_url("https://host.com/a@b/c"),
            "https://host.com/a@b/c"
        );
    }
}
