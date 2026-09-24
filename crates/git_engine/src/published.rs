use crate::RepoHandle;
use gix::ObjectId;
use gix::hashtable::{HashMap, HashSet};

type Walker<'repo, 'cache> = gix::revwalk::Graph<'repo, 'cache, ()>;

impl RepoHandle {
    /// `is_published` without spawning git: trivially without remote branches, else pruned
    /// by commit-graph generation numbers the way `git for-each-ref --contains` prunes.
    /// `None` when remotes exist but no commit-graph does, or `rev` does not resolve here:
    /// the caller asks git, which also words the refusal.
    #[must_use]
    pub fn published_in_process(&self, rev: &str) -> Option<bool> {
        let target = self.commit_of(rev)?;
        if !self.has_remote_refs()? {
            return Some(false);
        }
        let cache = self.commit_graph()?;
        let tips: Vec<ObjectId> = self
            .remote_refs(&cache)?
            .into_iter()
            .map(|(_, tip)| tip)
            .collect();
        let mut walker: Walker<'_, '_> = self.repo.revision_graph(Some(&cache));
        let floor = generation_floor(&mut walker, target)?;
        reaches(&mut walker, tips, target, floor)
    }

    /// `git for-each-ref --format=%(refname:short) --contains <rev> refs/remotes/` without
    /// spawning git: the same refs, in its order, under its short names. `None` on the
    /// terms of [`RepoHandle::published_in_process`].
    #[must_use]
    pub fn remote_refs_containing_in_process(&self, rev: &str) -> Option<Vec<String>> {
        let holding = self.remote_refs_holding(rev)?;
        Some(holding.into_iter().map(|(_, short)| short).collect())
    }

    /// The same, each as (full name, short name).
    pub(crate) fn remote_refs_holding(&self, rev: &str) -> Option<Vec<(String, String)>> {
        let target = self.commit_of(rev)?;
        if !self.has_remote_refs()? {
            return Some(Vec::new());
        }
        let cache = self.commit_graph()?;
        let remotes = self.remote_refs(&cache)?;
        let mut walker: Walker<'_, '_> = self.repo.revision_graph(Some(&cache));
        let floor = generation_floor(&mut walker, target)?;
        let tips: Vec<ObjectId> = remotes.iter().map(|(_, tip)| *tip).collect();
        let reach = reaching(&mut walker, &tips, target, floor)?;
        let holding: Vec<String> = remotes
            .into_iter()
            .filter(|(_, tip)| reach.get(tip).copied().unwrap_or(false))
            .map(|(name, _)| name)
            .collect();
        if holding.is_empty() {
            return Some(Vec::new());
        }
        let names = self.ref_names()?;
        let strict = self
            .repo
            .config_snapshot()
            .boolean("core.warnAmbiguousRefs")
            .unwrap_or(true);
        let exists = |name: &str| {
            if name.starts_with("refs/") {
                names.contains(name)
            } else {
                self.git_dir().join(name).is_file() || self.common_dir().join(name).is_file()
            }
        };
        Some(
            holding
                .into_iter()
                .map(|full| {
                    let short = shorten(&full, &exists, strict);
                    (full, short)
                })
                .collect(),
        )
    }

    pub(crate) fn commit_of(&self, rev: &str) -> Option<ObjectId> {
        let object = self.repo.rev_parse_single(rev).ok()?.object().ok()?;
        Some(object.peel_to_commit().ok()?.id)
    }

    fn commit_graph(&self) -> Option<gix::commitgraph::Graph> {
        match self.repo.commit_graph_if_enabled() {
            Ok(cache) => cache,
            Err(err) => {
                tracing::error!(error = ?err, context = "commit-graph for is_published");
                None
            }
        }
    }

    fn has_remote_refs(&self) -> Option<bool> {
        let platform = self.repo.references().ok()?;
        let mut remotes = platform.remote_branches().ok()?;
        Some(remotes.next().is_some())
    }

    /// Every remote-tracking ref with the commit it peels to, sorted by full name as git
    /// sorts them. A tip the commit-graph has is a commit and needs no object read (303
    /// peels were 19 ms); a ref that does not peel sends the question to git rather than
    /// dropping a tip.
    fn remote_refs(&self, graph: &gix::commitgraph::Graph) -> Option<Vec<(String, ObjectId)>> {
        let platform = self.repo.references().ok()?;
        let remotes = platform.remote_branches().ok()?;
        let mut refs: Vec<(String, ObjectId)> = remotes
            .map(|reference| {
                let mut reference = reference.ok()?;
                let name = reference.name().as_bstr().to_string();
                let tip = match reference.try_id() {
                    Some(id) if graph.lookup(id).is_some() => id.detach(),
                    _ => reference.peel_to_id().ok()?.detach(),
                };
                Some((name, tip))
            })
            .collect::<Option<_>>()?;
        refs.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        Some(refs)
    }

    fn ref_names(&self) -> Option<std::collections::HashSet<String>> {
        let platform = self.repo.references().ok()?;
        let all = platform.all().ok()?;
        Some(
            all.filter_map(Result::ok)
                .map(|reference| reference.name().as_bstr().to_string())
                .collect(),
        )
    }
}

/// `git rev-parse`'s rules, in git's order; `%(refname:short)` is decided by them.
const RULES: [(&str, &str); 6] = [
    ("", ""),
    ("refs/", ""),
    ("refs/tags/", ""),
    ("refs/heads/", ""),
    ("refs/remotes/", ""),
    ("refs/remotes/", "/HEAD"),
];

/// git's `shorten_unambiguous_ref`: the most specific rule whose short name no other rule
/// resolves (`strict`, git's default) or no earlier one does. So `refs/remotes/origin/HEAD`
/// is `origin`, and beside a local branch `origin/x`, `refs/remotes/origin/x` is
/// `remotes/origin/x`.
fn shorten(full: &str, exists: &dyn Fn(&str) -> bool, strict: bool) -> String {
    for (i, (prefix, suffix)) in RULES.iter().enumerate().skip(1).rev() {
        let Some(short) = full
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_suffix(suffix))
            .filter(|short| !short.is_empty())
        else {
            continue;
        };
        let checked = if strict { RULES.len() } else { i };
        let ambiguous = RULES[..checked]
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .any(|(_, (pre, suf))| exists(&format!("{pre}{short}{suf}")));
        if !ambiguous {
            return short.to_owned();
        }
    }
    full.to_owned()
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
    let mut seen = HashSet::default();
    let mut pending = tips;
    while let Some(id) = pending.pop() {
        if id == target {
            return Some(true);
        }
        if !seen.insert(id) {
            continue;
        }
        let Some(parents) = parents_above(walker, id, floor)? else {
            continue;
        };
        pending.extend(parents);
    }
    Some(false)
}

/// For every commit walked from `tips`, whether it reaches `target` — each commit decided
/// once, however many tips share its history: `git for-each-ref --contains` walks again
/// for every ref, which is what makes an old commit on hundreds of branches slow.
fn reaching(
    walker: &mut Walker<'_, '_>,
    tips: &[ObjectId],
    target: ObjectId,
    floor: u32,
) -> Option<HashMap<ObjectId, bool>> {
    let mut known: HashMap<ObjectId, bool> = HashMap::default();
    known.insert(target, true);
    let mut open: HashMap<ObjectId, Vec<ObjectId>> = HashMap::default();
    let mut pending: Vec<ObjectId> = tips.to_vec();
    while let Some(id) = pending.pop() {
        if known.contains_key(&id) {
            continue;
        }
        if let Some(parents) = open.remove(&id) {
            let reached = parents
                .iter()
                .any(|parent| known.get(parent).copied().unwrap_or(false));
            known.insert(id, reached);
            continue;
        }
        let Some(parents) = parents_above(walker, id, floor)? else {
            known.insert(id, false);
            continue;
        };
        // Decided on the way back, once every parent is.
        pending.push(id);
        pending.extend(parents.iter().filter(|parent| !known.contains_key(*parent)));
        open.insert(id, parents);
    }
    Some(known)
}

/// The parents of `id` worth walking, or `None` when nothing below it can be the target:
/// not a commit, missing in a shallow clone, or at or below the generation `floor`.
fn parents_above(
    walker: &mut Walker<'_, '_>,
    id: ObjectId,
    floor: u32,
) -> Option<Option<Vec<ObjectId>>> {
    let commit = match walker.try_lookup(&id) {
        Ok(Some(commit)) => commit,
        Ok(None) => return Some(None),
        Err(err) => {
            tracing::error!(error = ?err, context = "is_published walk");
            return None;
        }
    };
    if commit
        .generation()
        .is_some_and(|generation| generation <= floor)
    {
        return Some(None);
    }
    commit
        .iter_parents()
        .map(|parent| parent.ok())
        .collect::<Option<Vec<_>>>()
        .map(Some)
}
