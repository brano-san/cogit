//! The history walk: newest commit time first, ties by id. The same history always comes
//! out in the same order, so a walk can take what the last one read instead of reading the
//! objects again and still lay out exactly what a fresh walk would (R-301).

use crate::{CommitRow, RepoHandle};
use gix::ObjectId;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// What a walk listed of each commit: its row, commit time and every parent.
#[derive(Debug, Default, Clone)]
pub struct WalkedHistory {
    index: HashMap<ObjectId, (u32, i64, Vec<ObjectId>)>,
}

impl WalkedHistory {
    /// From the rows of a finished walk; a row whose ids do not parse is left out.
    pub fn from_rows<'a>(rows: impl IntoIterator<Item = &'a CommitRow>) -> Self {
        let mut history = Self::default();
        for (at, row) in rows.into_iter().enumerate() {
            let Ok(id) = ObjectId::from_hex(row.oid.as_bytes()) else {
                continue;
            };
            let parents: Option<Vec<ObjectId>> = row
                .parents
                .iter()
                .map(|parent| ObjectId::from_hex(parent.as_bytes()).ok())
                .collect();
            if let Some(parents) = parents {
                let at = u32::try_from(at).unwrap_or(u32::MAX);
                history.push(id, at, row.timestamp, parents);
            }
        }
        history
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.index.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Roughly what it holds, for a cache that counts bytes.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.index
            .values()
            .map(|(_, _, parents)| {
                size_of::<(ObjectId, (u32, i64, Vec<ObjectId>))>()
                    + size_of::<ObjectId>() * parents.len()
                    + 8
            })
            .sum()
    }

    pub(crate) fn push(&mut self, id: ObjectId, at: u32, time: i64, parents: Vec<ObjectId>) {
        self.index.insert(id, (at, time, parents));
    }

    fn get(&self, id: &ObjectId) -> Option<(u32, i64, &[ObjectId])> {
        let (at, time, parents) = self.index.get(id)?;
        Some((*at, *time, parents))
    }
}

/// Parents a walk never lists: those of a shallow commit and those it could not read. The
/// walk names them before the row that has them as parents goes out, so the layout can end
/// their lines in an arrow instead of waiting for a commit that never comes.
#[derive(Debug, Default)]
pub struct CutParents(std::cell::RefCell<HashSet<String>>);

impl CutParents {
    #[must_use]
    pub fn contains(&self, oid: &str) -> bool {
        self.0.borrow().contains(oid)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.borrow().is_empty()
    }

    pub(crate) fn insert(&self, id: ObjectId) {
        self.0.borrow_mut().insert(id.to_string());
    }
}

/// What the last graph walk listed: a walk takes time and parents from here instead of
/// reading the objects again (R-301).
#[derive(Debug, Clone, Copy)]
pub struct Reuse<'a> {
    pub history: &'a WalkedHistory,
}

impl Reuse<'_> {
    pub(crate) fn read(&self, id: &ObjectId) -> Option<(i64, Vec<ObjectId>)> {
        self.history
            .get(id)
            .map(|(_, time, parents)| (time, parents.to_vec()))
    }
}

struct Queued {
    time: i64,
    id: ObjectId,
    parents: Vec<ObjectId>,
}

impl Ord for Queued {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.time
            .cmp(&other.time)
            .then_with(|| other.id.cmp(&self.id))
    }
}

impl PartialOrd for Queued {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Queued {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time && self.id == other.id
    }
}

impl Eq for Queued {}

/// Newest commit time first, the smaller id first on a tie; each commit once.
pub(crate) struct ByTime<F> {
    queue: BinaryHeap<Queued>,
    seen: HashSet<ObjectId>,
    read: F,
    first_parent: bool,
    /// Sorted. Their parents are not in a shallow clone.
    shallow: Vec<ObjectId>,
}

impl<F> ByTime<F>
where
    F: FnMut(ObjectId) -> Option<(i64, Vec<ObjectId>)>,
{
    pub(crate) fn new(
        tips: impl IntoIterator<Item = ObjectId>,
        read: F,
        first_parent: bool,
        shallow: Vec<ObjectId>,
    ) -> Self {
        let mut walk = Self {
            queue: BinaryHeap::new(),
            seen: HashSet::new(),
            read,
            first_parent,
            shallow,
        };
        for tip in tips {
            walk.push(tip);
        }
        walk
    }

    fn push(&mut self, id: ObjectId) {
        if !self.seen.insert(id) {
            return;
        }
        if let Some((time, parents)) = (self.read)(id) {
            self.queue.push(Queued { time, id, parents });
        }
    }
}

impl<F> Iterator for ByTime<F>
where
    F: FnMut(ObjectId) -> Option<(i64, Vec<ObjectId>)>,
{
    type Item = (ObjectId, Vec<ObjectId>, i64);

    fn next(&mut self) -> Option<Self::Item> {
        let Queued { id, parents, time } = self.queue.pop()?;
        if self.shallow.binary_search(&id).is_err() {
            let follow = if self.first_parent { 1 } else { parents.len() };
            for parent in parents.iter().take(follow) {
                self.push(*parent);
            }
        }
        Some((id, parents, time))
    }
}

/// Commit time and parents: from the commit-graph file when there is one, else the object.
pub(crate) struct CommitReader<'a> {
    repo: &'a gix::Repository,
    graph: Option<gix::commitgraph::Graph>,
}

impl<'a> CommitReader<'a> {
    pub(crate) fn new(repo: &'a gix::Repository) -> Self {
        let graph = repo
            .commit_graph_if_enabled()
            .inspect_err(|err| tracing::warn!(error = %err, "commit-graph unreadable, not used"))
            .ok()
            .flatten();
        Self { repo, graph }
    }

    pub(crate) fn read(&self, id: ObjectId) -> Option<(i64, Vec<ObjectId>)> {
        if let Some(graph) = &self.graph
            && let Some(at) = graph.lookup(id)
        {
            let commit = graph.commit_at(at);
            let time = i64::try_from(commit.committer_timestamp()).unwrap_or(i64::MAX);
            let parents: std::result::Result<Vec<ObjectId>, _> = commit
                .iter_parents()
                .map(|parent| parent.map(|at| graph.id_at(at).to_owned()))
                .collect();
            if let Ok(parents) = parents {
                return Some((time, parents));
            }
        }
        let commit = self
            .repo
            .find_commit(id)
            .inspect_err(|err| tracing::warn!(error = %err, "skipping an unreadable commit"))
            .ok()?;
        let time = commit
            .time()
            .inspect_err(|err| tracing::warn!(error = %err, "skipping a commit without a time"))
            .ok()?;
        Some((
            time.seconds,
            commit.parent_ids().map(gix::Id::detach).collect(),
        ))
    }
}

impl RepoHandle {
    pub(crate) fn shallow_commits(&self) -> Vec<ObjectId> {
        match self.repo.shallow_commits() {
            Ok(Some(commits)) => {
                let mut commits: Vec<ObjectId> = commits.iter().copied().collect();
                commits.sort_unstable();
                commits
            }
            Ok(None) => Vec::new(),
            Err(err) => {
                tracing::error!(error = ?err, context = "graph walk: shallow file");
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ByTime;
    use gix::ObjectId;
    use std::collections::HashMap;

    fn id(n: u8) -> ObjectId {
        ObjectId::from_bytes_or_panic(&[n; 20])
    }

    /// `(id, time, parents)`.
    fn walk(
        commits: &[(u8, i64, &[u8])],
        tips: &[u8],
        first_parent: bool,
        shallow: &[u8],
    ) -> Vec<u8> {
        let by_id: HashMap<ObjectId, (i64, Vec<ObjectId>)> = commits
            .iter()
            .map(|(n, time, parents)| (id(*n), (*time, parents.iter().map(|p| id(*p)).collect())))
            .collect();
        let back: HashMap<ObjectId, u8> = commits.iter().map(|(n, ..)| (id(*n), *n)).collect();
        let tips: Vec<ObjectId> = tips.iter().map(|n| id(*n)).collect();
        let shallow = shallow.iter().map(|n| id(*n)).collect();
        ByTime::new(tips, |oid| by_id.get(&oid).cloned(), first_parent, shallow)
            .map(|(oid, ..)| back[&oid])
            .collect()
    }

    /// main 1 - 2 - 5 (merge of 4), side 1 - 3 - 4; 3 and 4 share a second with 2.
    const MERGED: &[(u8, i64, &[u8])] = &[
        (1, 10, &[]),
        (2, 20, &[1]),
        (3, 20, &[1]),
        (4, 20, &[3]),
        (5, 30, &[2, 4]),
    ];

    #[test]
    fn a_tie_goes_to_the_smaller_id_whichever_tip_came_first() {
        assert_eq!(walk(MERGED, &[5], false, &[]), [5, 2, 4, 3, 1]);
        assert_eq!(
            walk(MERGED, &[4, 2], false, &[]),
            walk(MERGED, &[2, 4], false, &[])
        );
    }

    #[test]
    fn first_parents_only_leave_the_merged_side_out() {
        assert_eq!(walk(MERGED, &[5], true, &[]), [5, 2, 1]);
    }

    #[test]
    fn a_shallow_commit_is_where_the_walk_ends() {
        assert_eq!(walk(MERGED, &[5], false, &[2, 3]), [5, 2, 4, 3]);
    }

    #[test]
    fn an_unreadable_commit_is_left_out_with_its_history() {
        let broken = &MERGED[1..];
        assert_eq!(walk(broken, &[5], false, &[]), [5, 2, 4, 3]);
    }
}
