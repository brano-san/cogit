use crate::RepoHandle;
use gix::ObjectId;
use gix::hashtable::HashSet;

type Walker<'repo, 'cache> = gix::revwalk::Graph<'repo, 'cache, ()>;

impl RepoHandle {
    /// `is_published` without spawning git: trivially without remote branches, else pruned
    /// by commit-graph generation numbers the way `git for-each-ref --contains` prunes.
    /// `None` when remotes exist but no commit-graph does, or `rev` does not resolve here:
    /// the caller asks git, which also words the refusal.
    #[must_use]
    pub fn published_in_process(&self, rev: &str) -> Option<bool> {
        let target = self.commit_of(rev)?;
        let tips: Vec<ObjectId> = self
            .remote_refs()?
            .into_iter()
            .map(|(_, tip)| tip)
            .collect();
        if tips.is_empty() {
            return Some(false);
        }
        let cache = self.commit_graph()?;
        let mut walker: Walker<'_, '_> = self.repo.revision_graph(Some(&cache));
        let floor = generation_floor(&mut walker, target)?;
        reaches(&mut walker, tips, target, floor, &mut HashSet::default())
    }

    /// `git for-each-ref --format=%(refname:short) --contains <rev> refs/remotes/` without
    /// spawning git: the same refs, in its order, under its short names. `None` on the
    /// terms of [`RepoHandle::published_in_process`].
    #[must_use]
    pub fn remote_refs_containing_in_process(&self, rev: &str) -> Option<Vec<String>> {
        let target = self.commit_of(rev)?;
        let remotes = self.remote_refs()?;
        if remotes.is_empty() {
            return Some(Vec::new());
        }
        let cache = self.commit_graph()?;
        let mut walker: Walker<'_, '_> = self.repo.revision_graph(Some(&cache));
        let floor = generation_floor(&mut walker, target)?;
        // History one tip was walked through without finding the target is not walked again.
        let mut dead = HashSet::default();
        let mut holding = Vec::new();
        for (name, tip) in remotes {
            if reaches(&mut walker, vec![tip], target, floor, &mut dead)? {
                holding.push(name);
            }
        }
        if holding.is_empty() {
            return Some(holding);
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
                .iter()
                .map(|full| shorten(full, &exists, strict))
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

    /// Every remote-tracking ref with the commit it peels to, sorted by full name as git
    /// sorts them. A ref that does not peel sends the question to git rather than
    /// dropping a tip.
    fn remote_refs(&self) -> Option<Vec<(String, ObjectId)>> {
        let platform = self.repo.references().ok()?;
        let remotes = platform.remote_branches().ok()?;
        let mut refs: Vec<(String, ObjectId)> = remotes
            .map(|reference| {
                let mut reference = reference.ok()?;
                let name = reference.name().as_bstr().to_string();
                Some((name, reference.peel_to_id().ok()?.detach()))
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
/// which is not the target cannot have it as an ancestor, so its history is skipped, as
/// is `dead` history; a walk that finds nothing adds what it saw to `dead`.
fn reaches(
    walker: &mut Walker<'_, '_>,
    tips: Vec<ObjectId>,
    target: ObjectId,
    floor: u32,
    dead: &mut HashSet<ObjectId>,
) -> Option<bool> {
    let mut seen = HashSet::default();
    let mut pending = tips;
    while let Some(id) = pending.pop() {
        if id == target {
            return Some(true);
        }
        if dead.contains(&id) || !seen.insert(id) {
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
    dead.extend(seen);
    Some(false)
}
