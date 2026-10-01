//! The Pull and Push dialogs (F-610): what they ask of git, the notes that travel beside
//! branches, and the choices a repository remembers.

use crate::{GitError, RepoHandle, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum PullMethod {
    #[default]
    Merge,
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
    pub method: PullMethod,
    /// With `Merge`: refuse a merge commit (Preferences ▸ Pull).
    pub ff_only: bool,
    pub fetch: FetchOptions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PushOptions {
    pub remote: String,
    pub local: String,
    pub branch: String,
    pub set_upstream: bool,
    pub tags: TagsMode,
    pub notes: bool,
    pub force_with_lease: bool,
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
    /// `None`: on while the branch has no upstream.
    pub push_set_upstream: Option<bool>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PushCommit {
    pub oid: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PushPreview {
    pub total: u32,
    pub commits: Vec<PushCommit>,
    /// Local notes the remote lacks, from the notes last fetched from it; `None` while
    /// nothing was fetched to compare with.
    pub notes_unpushed: Option<u32>,
    pub has_local_notes: bool,
}

const LOCAL_NOTES: &str = "refs/notes/";

fn notes_mirror(remote: &str) -> String {
    format!("refs/notes-remote/{remote}/")
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

#[must_use]
pub fn pull_args<'a>(remote: &'a str, options: &PullOptions) -> Vec<&'a str> {
    let mut args = vec!["pull", "--progress", "--prune"];
    if options.fetch.tags {
        args.extend(["--tags", "--force"]);
    }
    args.push(match (options.method, options.ff_only) {
        (PullMethod::Rebase, _) => "--rebase",
        (PullMethod::Merge, true) => "--ff-only",
        (PullMethod::Merge, false) => "--no-rebase",
    });
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
    if options.force_with_lease {
        // As the toolbar's forced push does: a lease alone passes when a background fetch
        // moved the tracking ref.
        args.push("--force-with-lease".into());
        args.push("--force-if-includes".into());
    }
    args.push(options.remote.clone());
    args.push(format!(
        "refs/heads/{}:refs/heads/{}",
        options.local, options.branch
    ));
    args
}

impl RepoHandle {
    pub fn fetch_options(
        &self,
        remote: &str,
        options: FetchOptions,
        token: impl Fn(&str) -> Option<String>,
        mut on_line: impl FnMut(&str),
    ) -> Result<NotesFetch> {
        let notes = if options.notes {
            self.fetch_notes(remote, &token, &mut on_line)?
        } else {
            NotesFetch::default()
        };
        let header = self.auth_for_fetch(remote, &token);
        self.stream(&header, &fetch_args(remote, options.tags), &mut on_line)?;
        Ok(notes)
    }

    /// Pull; notes come first, since they touch no working tree.
    pub fn pull_options(
        &self,
        remote: &str,
        options: PullOptions,
        token: impl Fn(&str) -> Option<String>,
        mut on_line: impl FnMut(&str),
    ) -> Result<NotesFetch> {
        let notes = if options.fetch.notes {
            self.fetch_notes(remote, &token, &mut on_line)?
        } else {
            NotesFetch::default()
        };
        let header = self.auth_for_fetch(remote, &token);
        self.stream(&header, &pull_args(remote, &options), &mut on_line)?;
        Ok(notes)
    }

    /// The branch, then the notes; a notes refusal is an outcome, not a failure.
    pub fn push_options(
        &self,
        options: &PushOptions,
        token: impl Fn(&str) -> Option<String>,
        mut on_line: impl FnMut(&str),
    ) -> Result<PushOutcome> {
        let header = self.auth_for_push(&options.remote, &token);
        let args = push_args(options);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        self.stream(&header, &args, &mut on_line)?;
        if options.notes {
            return self.push_notes(&options.remote, token, on_line);
        }
        Ok(PushOutcome {
            notes_rejected: None,
        })
    }

    /// `refs/notes/*` to the same names. Never forced: a remote that moved its notes
    /// refuses, and the dialog offers Fetch notes and merge.
    pub fn push_notes(
        &self,
        remote: &str,
        token: impl Fn(&str) -> Option<String>,
        mut on_line: impl FnMut(&str),
    ) -> Result<PushOutcome> {
        let local = self.refs_under(LOCAL_NOTES);
        if local.is_empty() {
            return Ok(PushOutcome {
                notes_rejected: None,
            });
        }
        let header = self.auth_for_push(remote, &token);
        let refspec = "refs/notes/*:refs/notes/*";
        let args = ["push", "--progress", remote, refspec];
        match self.stream(&header, &args, &mut on_line) {
            Ok(()) => {
                for (name, id) in local {
                    let mirror = format!("{}{}", notes_mirror(remote), &name[LOCAL_NOTES.len()..]);
                    self.run_git(&["update-ref", &mirror, &id])?;
                }
                Ok(PushOutcome {
                    notes_rejected: None,
                })
            }
            Err(GitError::Command(failure)) if is_non_fast_forward(&failure) => Ok(PushOutcome {
                notes_rejected: Some(output_text(&failure)),
            }),
            Err(err) => Err(err),
        }
    }

    /// Fetches the remote's notes beside ours (`refs/notes-remote/<remote>/*`), moves
    /// every namespace that only fell behind, and names those that diverged: they are
    /// never overwritten.
    pub fn fetch_notes(
        &self,
        remote: &str,
        token: &impl Fn(&str) -> Option<String>,
        on_line: &mut impl FnMut(&str),
    ) -> Result<NotesFetch> {
        let header = self.auth_for_fetch(remote, token);
        let mirror = notes_mirror(remote);
        let spec = format!("+{LOCAL_NOTES}*:{mirror}*");
        self.stream(&header, &["fetch", "--progress", remote, &spec], on_line)?;

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
    /// the remote branch, or, while it is unknown, from any branch of that remote.
    #[must_use]
    pub fn push_preview(
        &self,
        local: &str,
        remote: &str,
        branch: &str,
        limit: usize,
    ) -> PushPreview {
        let (total, commits) = self.unpushed_commits(local, remote, branch, limit);
        let mirror = notes_mirror(remote);
        let ours = self.refs_under(LOCAL_NOTES);
        let theirs = self.refs_under(&mirror);
        PushPreview {
            total,
            commits,
            has_local_notes: !ours.is_empty(),
            notes_unpushed: (!theirs.is_empty() && !ours.is_empty())
                .then(|| self.count_notes_unpushed(&ours, &theirs, &mirror)),
        }
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
        let Ok(walk) = self.repo.rev_walk(Some(tip)).with_hidden(hidden).all() else {
            return (0, Vec::new());
        };
        let mut total = 0_u32;
        let mut commits = Vec::new();
        for info in walk.filter_map(std::result::Result::ok) {
            total = total.saturating_add(1);
            if commits.len() < limit {
                let summary = info
                    .object()
                    .ok()
                    .and_then(|commit| commit.message().ok().map(|m| m.summary().to_string()))
                    .unwrap_or_default();
                commits.push(PushCommit {
                    oid: info.id.to_string(),
                    summary,
                });
            }
        }
        (total, commits)
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
        token: &impl Fn(&str) -> Option<String>,
    ) -> Option<String> {
        self.auth_arg(remote, gix::remote::Direction::Fetch, token)
    }

    fn auth_for_push(
        &self,
        remote: &str,
        token: &impl Fn(&str) -> Option<String>,
    ) -> Option<String> {
        self.auth_arg(remote, gix::remote::Direction::Push, token)
    }

    fn stream(
        &self,
        header: &Option<String>,
        args: &[&str],
        on_line: &mut impl FnMut(&str),
    ) -> Result<()> {
        let mut all: Vec<&str> = Vec::new();
        if let Some(value) = header {
            all.extend(["-c", value.as_str()]);
        }
        all.extend_from_slice(args);
        self.run_streaming(&all, |line| on_line(line))
    }

    /// What this repository remembers for the dialogs, from its own config only.
    pub fn network_defaults(&self) -> Result<NetworkDefaults> {
        let listing = self.run_git_reading(&["config", "--local", "--list"])?;
        let mut defaults = NetworkDefaults::default();
        for line in listing.stdout.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key {
                "cogit.pullmethod" => {
                    defaults.pull_method = if value == "rebase" {
                        PullMethod::Rebase
                    } else {
                        PullMethod::Merge
                    };
                }
                "cogit.pulltags" => defaults.pull_tags = value == "true",
                "cogit.pullnotes" => defaults.pull_notes = value == "true",
                "cogit.pushtags" => {
                    defaults.push_tags = match value {
                        "follow" => TagsMode::Follow,
                        "all" => TagsMode::All,
                        _ => TagsMode::None,
                    };
                }
                "cogit.pushnotes" => defaults.push_notes = value == "true",
                "cogit.pushsetupstream" => defaults.push_set_upstream = Some(value == "true"),
                _ => {}
            }
        }
        Ok(defaults)
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
        match defaults.push_set_upstream {
            Some(on) => pairs.push(("cogit.pushSetUpstream", flag(on))),
            None => {
                // Absent means automatic; clear it only when it is there.
                let present = self
                    .run_git_reading(&["config", "--local", "--list"])?
                    .stdout
                    .lines()
                    .any(|line| line.starts_with("cogit.pushsetupstream="));
                if present {
                    self.run_git(&["config", "--local", "--unset-all", "cogit.pushSetUpstream"])?;
                }
            }
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

fn is_non_fast_forward(failure: &crate::GitCommandError) -> bool {
    let text = output_text(failure);
    text.contains("[rejected]") || text.contains("non-fast-forward") || text.contains("fetch first")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push(f: impl FnOnce(&mut PushOptions)) -> Vec<String> {
        let mut options = PushOptions {
            remote: "origin".into(),
            local: "main".into(),
            branch: "main".into(),
            set_upstream: false,
            tags: TagsMode::None,
            notes: false,
            force_with_lease: false,
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
        });
        assert_eq!(
            args,
            [
                "push",
                "--progress",
                "--set-upstream",
                "--follow-tags",
                "--force-with-lease",
                "--force-if-includes",
                "origin",
                "refs/heads/main:refs/heads/main"
            ]
        );
        assert!(push(|o| o.tags = TagsMode::All).contains(&"--tags".to_owned()));
    }

    #[test]
    fn pull_picks_one_way_to_integrate() {
        let mut options = PullOptions::default();
        assert!(pull_args("o", &options).contains(&"--no-rebase"));
        options.ff_only = true;
        assert!(pull_args("o", &options).contains(&"--ff-only"));
        options.method = PullMethod::Rebase;
        let args = pull_args("o", &options);
        assert!(args.contains(&"--rebase") && !args.contains(&"--ff-only"));
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
        assert!(pull_args("o", &options).contains(&"--force"));
    }
}
