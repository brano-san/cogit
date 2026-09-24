use crate::RepoHandle;
use gix::ObjectId;

type Walker<'repo, 'cache> = gix::revwalk::Graph<'repo, 'cache, ()>;

impl RepoHandle {
    /// `is_published` without spawning git: trivially without remote branches, else pruned
    /// by commit-graph generation numbers the way `git for-each-ref --contains` prunes.
    /// `None` when remotes exist but no commit-graph does, or `rev` does not resolve here:
    /// the caller asks git, which also words the refusal.
    #[must_use]
    pub fn published_in_process(&self, rev: &str) -> Option<bool> {
        let target = self
            .repo
            .rev_parse_single(rev)
            .ok()?
            .object()
            .ok()?
            .peel_to_commit()
            .ok()?
            .id;
        let tips = self.remote_tips()?;
        if tips.is_empty() {
            return Some(false);
        }
        let cache = match self.repo.commit_graph_if_enabled() {
            Ok(cache) => cache?,
            Err(err) => {
                tracing::error!(error = ?err, context = "commit-graph for is_published");
                return None;
            }
        };
        let mut walker: Walker<'_, '_> = self.repo.revision_graph(Some(&cache));
        let floor = generation_floor(&mut walker, target)?;
        reaches(&mut walker, tips, target, floor)
    }

    fn remote_tips(&self) -> Option<Vec<ObjectId>> {
        let platform = self.repo.references().ok()?;
        let remotes = platform.remote_branches().ok()?;
        // A ref that does not peel sends the question to git rather than dropping a tip.
        remotes
            .map(|reference| Some(reference.ok()?.peel_to_id().ok()?.detach()))
            .collect()
    }
}

/// A lower bound of `target`'s generation: exact when the commit-graph has it, else the
/// first graphed commit down its first-parent chain plus the steps to it. A lower bound
/// only prunes less, never wrongly.
fn generation_floor(walker: &mut Walker<'_, '_>, target: ObjectId) -> Option<u32> {
    let mut steps = 0u32;
    let mut at = target;
    loop {
        let commit = walker.try_lookup(&at).ok()??;
        if let Some(generation) = commit.generation() {
            return Some(generation.saturating_add(steps));
        }
        match commit.iter_parents().next() {
            Some(parent) => at = parent.ok()?,
            None => return Some(steps.saturating_add(1)),
        }
        steps = steps.saturating_add(1);
    }
}

/// Whether any tip reaches `target`. A commit whose generation is at most `floor` and
/// which is not the target cannot have it as an ancestor, so its history is skipped.
fn reaches(
    walker: &mut Walker<'_, '_>,
    tips: Vec<ObjectId>,
    target: ObjectId,
    floor: u32,
) -> Option<bool> {
    let mut seen = gix::hashtable::HashSet::default();
    let mut pending = tips;
    while let Some(id) = pending.pop() {
        if id == target {
            return Some(true);
        }
        if !seen.insert(id) {
            continue;
        }
        let commit = match walker.try_lookup(&id) {
            Ok(Some(commit)) => commit,
            // Not a commit, or missing in a shallow clone: nothing to walk.
            Ok(None) => continue,
            Err(err) => {
                tracing::error!(error = ?err, context = "is_published walk");
                return None;
            }
        };
        if commit
            .generation()
            .is_some_and(|generation| generation <= floor)
        {
            continue;
        }
        for parent in commit.iter_parents() {
            pending.push(parent.ok()?);
        }
    }
    Some(false)
}
