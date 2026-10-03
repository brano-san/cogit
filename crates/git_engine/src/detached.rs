use crate::{BranchKind, RepoHandle, Result};
use serde::Serialize;
use std::collections::BTreeMap;

/// What went; empty: nothing qualified and nothing ran.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Pushed {
    pub branches: Vec<String>,
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

    /// Branches ahead of their upstream to their push remotes; one `git push` per remote,
    /// never forced. Tags are not pushed (F-710).
    pub fn push_pushable(
        &self,
        token: impl Fn(&str) -> Option<String>,
        mut on_line: impl FnMut(&str),
    ) -> Result<Pushed> {
        let branches = self.pushable_branches()?;
        let mut by_remote: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for branch in &branches {
            by_remote
                .entry(&branch.remote)
                .or_default()
                .push(&branch.refspec);
        }
        for (remote, refspecs) in by_remote {
            let auth = self.auth_env(remote, gix::remote::Direction::Push, &token);
            let mut args = vec!["push", "--progress", remote];
            args.extend(refspecs);
            self.run_streaming(&args, &auth, &mut on_line)?;
        }
        Ok(Pushed {
            branches: branches.into_iter().map(|branch| branch.name).collect(),
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
