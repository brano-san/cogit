use crate::{BranchKind, RepoHandle, Result};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};

/// What went; both empty: nothing qualified and nothing ran.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Pushed {
    pub branches: Vec<String>,
    pub tags: Vec<String>,
}

struct PushableBranch {
    name: String,
    remote: String,
    refspec: String,
}

impl RepoHandle {
    /// A diverged branch is left out: only a forced push could send it.
    fn pushable_branches(&self) -> Result<Vec<PushableBranch>> {
        let mut out = Vec::new();
        for branch in self.branches()? {
            let behind = branch.push_target.is_none() && branch.behind > 0;
            if branch.kind != BranchKind::Local
                || branch.upstream.is_none()
                || branch.ahead == 0
                || behind
            {
                continue;
            }
            let Ok(full) = gix::refs::FullName::try_from(branch.full_name.as_str()) else {
                continue;
            };
            let Some(remote) = branch.push_remote.clone() else {
                continue;
            };
            let Some(Ok(target)) = self
                .repo
                .branch_remote_ref_name(full.as_ref(), gix::remote::Direction::Push)
            else {
                continue;
            };
            out.push(PushableBranch {
                refspec: format!("{}:{}", branch.full_name, target.as_bstr()),
                name: branch.name,
                remote,
            });
        }
        Ok(out)
    }

    /// By name: a tag the server has elsewhere would need force.
    fn unpushed_tags(
        &self,
        remote: &str,
        token: impl Fn(&str) -> Option<String>,
    ) -> Result<Vec<String>> {
        let local = self.ref_tips(&["refs/tags/"]);
        if local.is_empty() {
            return Ok(Vec::new());
        }
        let auth = self.auth_env(remote, gix::remote::Direction::Fetch, &token);
        let listed =
            self.run_streaming_read(&["ls-remote", "--tags", "--refs", remote], &auth, |_| {})?;
        let there: HashSet<&str> = listed
            .lines()
            .filter_map(|line| line.split_once('\t').map(|(_, name)| name.trim()))
            .collect();
        Ok(local
            .into_iter()
            .filter(|(name, _)| !there.contains(name.as_str()))
            .filter_map(|(name, _)| name.strip_prefix("refs/tags/").map(str::to_owned))
            .collect())
    }

    /// Branches ahead of their upstream to their push remotes, tags `tag_remote` lacks to
    /// it; one `git push` per remote, never forced.
    pub fn push_pushable(
        &self,
        tag_remote: &str,
        token: impl Fn(&str) -> Option<String>,
        mut on_line: impl FnMut(&str),
    ) -> Result<Pushed> {
        let branches = self.pushable_branches()?;
        let tags = self.unpushed_tags(tag_remote, &token)?;
        let mut by_remote: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for branch in &branches {
            by_remote
                .entry(&branch.remote)
                .or_default()
                .push(branch.refspec.clone());
        }
        for tag in &tags {
            by_remote
                .entry(tag_remote)
                .or_default()
                .push(format!("refs/tags/{tag}:refs/tags/{tag}"));
        }
        for (remote, refspecs) in by_remote {
            let auth = self.auth_env(remote, gix::remote::Direction::Push, &token);
            let mut args = vec!["push", "--progress", remote];
            args.extend(refspecs.iter().map(String::as_str));
            self.run_streaming(&args, &auth, &mut on_line)?;
        }
        Ok(Pushed {
            branches: branches.into_iter().map(|branch| branch.name).collect(),
            tags,
        })
    }
}

/// Detached, a pull only fetches: there is no branch to merge into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum PullOutcome {
    Pulled,
    FetchedDetached,
}
