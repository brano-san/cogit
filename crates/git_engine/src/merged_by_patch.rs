//! Branches the target took in by squash or rebase: no ancestry, the same patches (F-949).

use crate::{BranchKind, GitError, RepoHandle, Result};
use gix::ObjectId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

/// Commits walked on either side of the merge base before a branch is "not checked".
pub const MERGE_SCAN_CAP: usize = 2000;
/// Patches per `diff-tree`: bounds its output in memory and lets a scan stop between them.
const BATCH: usize = 256;
/// Past these the cache starts over rather than grow for as long as the app runs.
const KINDS_KEPT: usize = 20_000;
const PATCHES_KEPT: usize = 200_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum MergeKind {
    Merged,
    Squash,
    Rebase,
    NotMerged,
    /// More than [`MERGE_SCAN_CAP`] commits on a side since the merge base.
    NotChecked,
}

impl MergeKind {
    #[must_use]
    pub fn is_merged(self) -> bool {
        matches!(self, Self::Merged | Self::Squash | Self::Rebase)
    }

    /// `git branch -d` refuses a tip the target does not contain.
    #[must_use]
    pub fn needs_force(self) -> bool {
        matches!(self, Self::Squash | Self::Rebase)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BranchMerge {
    pub name: String,
    /// The tip judged, so an answer for a branch that moved since is not shown.
    pub oid: String,
    pub kind: MergeKind,
}

/// What stays true as long as the objects exist: a kind per (tip, target), a patch-id per
/// commit and per squashed range.
#[derive(Debug, Default)]
pub struct MergeCache {
    kinds: HashMap<(ObjectId, ObjectId), MergeKind>,
    patches: HashMap<Patch, Option<ObjectId>>,
}

impl MergeCache {
    pub fn absorb(&mut self, other: Self) {
        self.kinds.extend(other.kinds);
        self.patches.extend(other.patches);
        self.trim();
    }

    fn trim(&mut self) {
        if self.kinds.len() > KINDS_KEPT {
            self.kinds.clear();
        }
        if self.patches.len() > PATCHES_KEPT {
            self.patches.clear();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Patch {
    Commit(ObjectId),
    Squash { base: ObjectId, tip: ObjectId },
}

struct Pending {
    at: usize,
    tip: ObjectId,
    base: ObjectId,
    branch: Vec<ObjectId>,
    target: std::rc::Rc<Vec<ObjectId>>,
}

enum Scan {
    Done(MergeKind),
    Compare(Pending),
}

impl RepoHandle {
    /// HEAD's verdict on every local branch but its own, merged ones only. Branches still
    /// unjudged when `cancelled` turns true are left out.
    pub fn branch_merges(
        &self,
        cache: &mut MergeCache,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<BranchMerge>> {
        let Ok(head) = self.repo.head_id() else {
            return Ok(Vec::new());
        };
        let head = head.detach();
        let branches: Vec<(String, ObjectId)> = self
            .branches_without_divergence()?
            .into_iter()
            .filter(|branch| branch.kind == BranchKind::Local && !branch.is_head)
            .filter_map(|branch| {
                Some((branch.name, ObjectId::from_hex(branch.oid.as_bytes()).ok()?))
            })
            .filter(|(_, tip)| *tip != head)
            .collect();
        let tips: Vec<ObjectId> = branches.iter().map(|(_, tip)| *tip).collect();
        let kinds = self.merge_kinds(head, &tips, MERGE_SCAN_CAP, cache, cancelled)?;
        Ok(branches
            .into_iter()
            .zip(kinds)
            .filter_map(|((name, tip), kind)| {
                let kind = kind.filter(|kind| *kind != MergeKind::NotMerged)?;
                Some(BranchMerge {
                    name,
                    oid: tip.to_string(),
                    kind,
                })
            })
            .collect())
    }

    /// How `target` took in each of `tips`, in order; `None` for those not reached before
    /// `cancelled`. `cap`: [`MERGE_SCAN_CAP`] but in tests.
    pub fn merge_kinds(
        &self,
        target: ObjectId,
        tips: &[ObjectId],
        cap: usize,
        cache: &mut MergeCache,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<Option<MergeKind>>> {
        let mut kinds = vec![None; tips.len()];
        let mut pending = Vec::new();
        let mut target_sides = HashMap::new();
        for (at, &tip) in tips.iter().enumerate() {
            if let Some(&kind) = cache.kinds.get(&(tip, target)) {
                kinds[at] = Some(kind);
                continue;
            }
            if cancelled() {
                return Ok(kinds);
            }
            match self.scan(at, tip, target, cap, &mut target_sides) {
                Scan::Done(kind) => {
                    cache.kinds.insert((tip, target), kind);
                    kinds[at] = Some(kind);
                }
                Scan::Compare(each) => pending.push(each),
            }
        }
        if pending.is_empty() {
            return Ok(kinds);
        }

        let mut wanted = Vec::new();
        let mut seen = HashSet::new();
        for each in &pending {
            let squash = Patch::Squash {
                base: each.base,
                tip: each.tip,
            };
            let commits = each.branch.iter().chain(each.target.iter());
            for patch in commits.map(|id| Patch::Commit(*id)).chain([squash]) {
                if !cache.patches.contains_key(&patch) && seen.insert(patch) {
                    wanted.push(patch);
                }
            }
        }
        let started = std::time::Instant::now();
        for batch in wanted.chunks(BATCH) {
            if cancelled() {
                return Ok(kinds);
            }
            let ids = self.patch_ids(batch, target.kind())?;
            cache.patches.extend(batch.iter().copied().zip(ids));
        }
        if !wanted.is_empty() {
            tracing::info!(elapsed = ?started.elapsed(), patches = wanted.len(), branches = pending.len(), "patch-ids for merged branches");
        }

        let patch = |key: Patch| cache.patches.get(&key).copied().flatten();
        let mut found = Vec::with_capacity(pending.len());
        for each in &pending {
            let target_ids: HashSet<ObjectId> = each
                .target
                .iter()
                .filter_map(|id| patch(Patch::Commit(*id)))
                .collect();
            let branch_ids: Vec<Option<ObjectId>> = each
                .branch
                .iter()
                .map(|id| patch(Patch::Commit(*id)))
                .collect();
            let squash = patch(Patch::Squash {
                base: each.base,
                tip: each.tip,
            });
            found.push((
                each.at,
                each.tip,
                classify(&branch_ids, squash, &target_ids),
            ));
        }
        for (at, tip, kind) in found {
            cache.kinds.insert((tip, target), kind);
            kinds[at] = Some(kind);
        }
        cache.trim();
        Ok(kinds)
    }

    fn scan(
        &self,
        at: usize,
        tip: ObjectId,
        target: ObjectId,
        cap: usize,
        target_sides: &mut HashMap<ObjectId, Option<std::rc::Rc<Vec<ObjectId>>>>,
    ) -> Scan {
        if tip == target || self.reaches_in_process(target, tip) == Some(true) {
            return Scan::Done(MergeKind::Merged);
        }
        let Ok(base) = self.repo.merge_base(tip, target).map(gix::Id::detach) else {
            return Scan::Done(MergeKind::NotMerged);
        };
        if base == tip {
            return Scan::Done(MergeKind::Merged);
        }
        let target_side = target_sides
            .entry(base)
            .or_insert_with(|| {
                self.only_reachable_within(target, base, cap)
                    .map(std::rc::Rc::new)
            })
            .clone();
        let Some(target_side) = target_side else {
            return Scan::Done(MergeKind::NotChecked);
        };
        if target_side.is_empty() {
            return Scan::Done(MergeKind::NotMerged);
        }
        let Some(branch) = self.only_reachable_within(tip, base, cap) else {
            return Scan::Done(MergeKind::NotChecked);
        };
        Scan::Compare(Pending {
            at,
            tip,
            base,
            branch,
            target: target_side,
        })
    }

    /// One patch-id per patch, `None` for an empty diff or a merge commit. `diff-tree
    /// --always` heads every patch with its first oid; each head is swapped for its
    /// position, so `patch-id` names patches that share a tip apart.
    fn patch_ids(&self, batch: &[Patch], hash: gix::hash::Kind) -> Result<Vec<Option<ObjectId>>> {
        let mut input = String::new();
        let mut heads = Vec::with_capacity(batch.len());
        for patch in batch {
            let (line, head) = match *patch {
                Patch::Commit(id) => (id.to_string(), id),
                Patch::Squash { base, tip } => (format!("{tip} {base}"), tip),
            };
            let _ = writeln!(input, "{line}");
            heads.push(head.to_string());
        }
        let diff = self.read_bytes_fed(
            &[
                "diff-tree",
                "--stdin",
                "-p",
                "--always",
                "--root",
                "--no-renames",
                "--no-ext-diff",
                "--no-textconv",
            ],
            &[("LC_ALL", "C")],
            Some(input.as_bytes()),
        )?;
        let width = hash.len_in_hex();
        let numbered = number_heads(&diff, &heads, width);
        let ids = self.read_bytes_fed(
            &["patch-id", "--stable"],
            &[("LC_ALL", "C")],
            Some(&numbered),
        )?;
        let mut found = vec![None; batch.len()];
        for line in String::from_utf8_lossy(&ids).lines() {
            let mut parts = line.split_whitespace();
            let (Some(id), Some(at)) = (parts.next(), parts.next()) else {
                continue;
            };
            let id = ObjectId::from_hex(id.as_bytes())
                .map_err(|err| GitError::Internal(format!("patch-id printed {line:?}: {err}")))?;
            let at = usize::from_str_radix(at, 16)
                .map_err(|err| GitError::Internal(format!("patch-id printed {line:?}: {err}")))?;
            if let Some(slot) = found.get_mut(at) {
                *slot = Some(id);
            }
        }
        Ok(found)
    }
}

/// `diff` with the head line of patch `n` replaced by `n` in hex, `width` digits. A body
/// line never is a bare oid: each starts with `diff`, `index`, `@@`, a sign or a space.
fn number_heads(diff: &[u8], heads: &[String], width: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(diff.len());
    let mut next = 0;
    for line in diff.split_inclusive(|byte| *byte == b'\n') {
        let bare = line.strip_suffix(b"\n").unwrap_or(line);
        if heads.get(next).is_some_and(|head| head.as_bytes() == bare) {
            out.extend_from_slice(format!("{next:0width$x}\n").as_bytes());
            next += 1;
        } else {
            out.extend_from_slice(line);
        }
    }
    out
}

/// Rebase first: a branch of one commit squashed reads the same either way.
fn classify(
    branch: &[Option<ObjectId>],
    squash: Option<ObjectId>,
    target: &HashSet<ObjectId>,
) -> MergeKind {
    let mut ids = branch.iter().flatten().peekable();
    if ids.peek().is_some() && ids.all(|id| target.contains(id)) {
        return MergeKind::Rebase;
    }
    if squash.is_some_and(|id| target.contains(&id)) {
        return MergeKind::Squash;
    }
    MergeKind::NotMerged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u8) -> ObjectId {
        ObjectId::from_bytes_or_panic(&[n; 20])
    }

    #[test]
    fn every_commit_found_is_a_rebase() {
        let target = HashSet::from([id(1), id(2), id(9)]);
        assert_eq!(
            classify(&[Some(id(1)), Some(id(2))], Some(id(5)), &target),
            MergeKind::Rebase
        );
    }

    #[test]
    fn one_commit_missing_is_not_merged() {
        let target = HashSet::from([id(1)]);
        assert_eq!(
            classify(&[Some(id(1)), Some(id(2))], Some(id(5)), &target),
            MergeKind::NotMerged
        );
    }

    #[test]
    fn the_whole_diff_as_one_commit_is_a_squash() {
        let target = HashSet::from([id(5)]);
        assert_eq!(
            classify(&[Some(id(1)), Some(id(2))], Some(id(5)), &target),
            MergeKind::Squash
        );
    }

    #[test]
    fn empty_and_merge_commits_neither_count_nor_make_a_rebase_alone() {
        let target = HashSet::from([id(1)]);
        assert_eq!(
            classify(&[None, Some(id(1))], None, &target),
            MergeKind::Rebase
        );
        assert_eq!(classify(&[None], None, &target), MergeKind::NotMerged);
    }

    #[test]
    fn heads_are_numbered_and_the_body_is_left_alone() {
        let heads = ["ab".repeat(20), "cd".repeat(20)];
        let diff = format!(
            "{}\ndiff --git a/x b/x\n+{}\n{}\n",
            heads[0], heads[1], heads[1]
        );
        let numbered = number_heads(diff.as_bytes(), &heads, 40);
        let text = String::from_utf8(numbered).unwrap();
        assert_eq!(
            text,
            format!(
                "{:040x}\ndiff --git a/x b/x\n+{}\n{:040x}\n",
                0, heads[1], 1
            )
        );
    }
}
