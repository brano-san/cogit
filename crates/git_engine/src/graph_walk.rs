//! The history walk: newest commit time first, ties by id. The same history always comes
//! out in the same order, so a walk can take what the last one read instead of reading the
//! objects again and still lay out exactly what a fresh walk would (R-301).

use crate::RepoHandle;
use gix::ObjectId;
use std::collections::{BinaryHeap, HashSet};

/// A commit's parents as a walk reads them: two fit inline.
pub type Parents = smallvec::SmallVec<[ObjectId; 2]>;

/// What a walk listed of each commit: its commit time and every parent.
#[derive(Debug, Default, Clone)]
pub struct WalkedHistory {
    index: gix::hashtable::HashMap<ObjectId, (i64, Parents)>,
}

impl WalkedHistory {
    /// Roughly what it holds, for a cache that counts bytes.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.index.capacity() * (size_of::<(ObjectId, (i64, Parents))>() + 1)
            + self
                .index
                .values()
                .filter(|(_, parents)| parents.spilled())
                .map(|(_, parents)| size_of::<ObjectId>() * parents.capacity())
                .sum::<usize>()
    }

    pub(crate) fn push(&mut self, id: ObjectId, time: i64, parents: &[ObjectId]) {
        self.index.insert(id, (time, Parents::from_slice(parents)));
    }
}

pub trait KnownCommits: std::fmt::Debug {
    fn time_and_parents(&self, id: &ObjectId) -> Option<(i64, Parents)>;
}

impl KnownCommits for WalkedHistory {
    fn time_and_parents(&self, id: &ObjectId) -> Option<(i64, Parents)> {
        let (time, parents) = self.index.get(id)?;
        Some((*time, parents.clone()))
    }
}

/// Links a walk never lists: from a shallow commit to its parents, and to a parent it could
/// not read. The walk names them before the row that has them as parents goes out, so the
/// layout can end their lines in an arrow instead of waiting for a commit that never comes.
/// A shallow boundary is the child's: another child of the same parent keeps its link.
#[derive(Debug, Default)]
pub struct CutParents {
    unreadable: std::cell::RefCell<HashSet<ObjectId>>,
    shallow: std::cell::RefCell<HashSet<ObjectId>>,
}

impl CutParents {
    #[must_use]
    pub fn hides(&self, child: &ObjectId, parent: &ObjectId) -> bool {
        self.shallow.borrow().contains(child) || self.unreadable.borrow().contains(parent)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.shallow.borrow().is_empty() && self.unreadable.borrow().is_empty()
    }

    pub(crate) fn insert(&self, id: ObjectId) {
        self.unreadable.borrow_mut().insert(id);
    }

    pub(crate) fn insert_shallow(&self, id: ObjectId) {
        self.shallow.borrow_mut().insert(id);
    }
}

/// What the last graph walk listed: a walk takes time and parents from here instead of
/// reading the objects again (R-301).
#[derive(Debug, Clone, Copy)]
pub struct Reuse<'a> {
    pub history: &'a dyn KnownCommits,
}

impl Reuse<'_> {
    pub(crate) fn read(&self, id: &ObjectId) -> Option<(i64, Parents)> {
        self.history.time_and_parents(id)
    }
}

struct Queued {
    time: i64,
    id: ObjectId,
    parents: Parents,
    level: u32,
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

#[derive(Debug, Clone)]
pub(crate) struct Horizon(std::rc::Rc<std::cell::Cell<u32>>);

impl Default for Horizon {
    fn default() -> Self {
        Self(std::rc::Rc::new(std::cell::Cell::new(u32::MAX)))
    }
}

impl Horizon {
    pub(crate) fn settles(&self, level: Option<u32>) -> bool {
        let horizon = self.0.get();
        match level {
            Some(level) => level >= horizon,
            None => horizon != u32::MAX,
        }
    }
}

struct Levels<L> {
    of: L,
    queued: std::collections::BTreeMap<u32, usize>,
    horizon: Horizon,
}

/// Newest commit time first, the smaller id first on a tie; each commit once.
pub(crate) struct ByTime<F, L = fn(&ObjectId) -> Option<u32>> {
    queue: BinaryHeap<Queued>,
    seen: gix::hashtable::HashSet<ObjectId>,
    read: F,
    first_parent: bool,
    /// Sorted. Their parents are not in a shallow clone.
    shallow: Vec<ObjectId>,
    levels: Option<Levels<L>>,
}

impl<F> ByTime<F>
where
    F: FnMut(ObjectId) -> Option<(i64, Parents)>,
{
    #[cfg(test)]
    pub(crate) fn new(
        tips: impl IntoIterator<Item = ObjectId>,
        read: F,
        first_parent: bool,
        shallow: Vec<ObjectId>,
    ) -> Self {
        Self::leveled(tips, read, None, first_parent, shallow)
    }
}

impl<F, L> ByTime<F, L>
where
    F: FnMut(ObjectId) -> Option<(i64, Parents)>,
    L: FnMut(&ObjectId) -> Option<u32>,
{
    pub(crate) fn leveled(
        tips: impl IntoIterator<Item = ObjectId>,
        read: F,
        levels: Option<(L, Horizon)>,
        first_parent: bool,
        shallow: Vec<ObjectId>,
    ) -> Self {
        let mut walk = Self {
            queue: BinaryHeap::new(),
            seen: gix::hashtable::HashSet::default(),
            read,
            first_parent,
            shallow,
            levels: levels.map(|(of, horizon)| Levels {
                of,
                queued: std::collections::BTreeMap::new(),
                horizon,
            }),
        };
        for tip in tips {
            walk.push(tip);
        }
        walk.raise_horizon();
        walk
    }

    fn push(&mut self, id: ObjectId) {
        if !self.seen.insert(id) {
            return;
        }
        if let Some((time, parents)) = (self.read)(id) {
            let level = match &mut self.levels {
                Some(levels) => {
                    let level = (levels.of)(&id).unwrap_or(u32::MAX);
                    *levels.queued.entry(level).or_insert(0) += 1;
                    level
                }
                None => u32::MAX,
            };
            self.queue.push(Queued {
                time,
                id,
                parents,
                level,
            });
        }
    }

    fn raise_horizon(&self) {
        if let Some(levels) = &self.levels {
            let top = levels
                .queued
                .last_key_value()
                .map_or(0, |(level, _)| *level);
            levels.horizon.0.set(top);
        }
    }
}

impl<F, L> Iterator for ByTime<F, L>
where
    F: FnMut(ObjectId) -> Option<(i64, Parents)>,
    L: FnMut(&ObjectId) -> Option<u32>,
{
    type Item = (ObjectId, Parents, i64);

    fn next(&mut self) -> Option<Self::Item> {
        let Queued {
            id,
            parents,
            time,
            level,
        } = self.queue.pop()?;
        if let Some(levels) = &mut self.levels
            && let std::collections::btree_map::Entry::Occupied(mut count) =
                levels.queued.entry(level)
        {
            *count.get_mut() -= 1;
            if *count.get() == 0 {
                count.remove();
            }
        }
        if self.shallow.binary_search(&id).is_err() {
            let follow = if self.first_parent { 1 } else { parents.len() };
            for parent in parents.iter().take(follow) {
                self.push(*parent);
            }
        }
        self.raise_horizon();
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

    pub(crate) fn level(&self, id: &ObjectId) -> Option<u32> {
        let graph = self.graph.as_ref()?;
        Some(graph.commit_at(graph.lookup(id)?).generation())
    }

    pub(crate) fn has_sound_levels(&self) -> bool {
        self.graph.as_ref().is_some_and(|graph| {
            graph.iter_commits().all(|commit| {
                let level = commit.generation();
                level != 0 && level < gix::commitgraph::GENERATION_NUMBER_MAX
            })
        })
    }

    pub(crate) fn read(&self, id: ObjectId) -> Option<(i64, Parents)> {
        self.read_with_level(id)
            .map(|(time, parents, _)| (time, parents))
    }

    pub(crate) fn read_with_level(&self, id: ObjectId) -> Option<(i64, Parents, u32)> {
        if let Some(graph) = &self.graph
            && let Some(at) = graph.lookup(id)
        {
            let commit = graph.commit_at(at);
            let parents: std::result::Result<Parents, _> = commit
                .iter_parents()
                .map(|parent| parent.map(|at| graph.id_at(at).to_owned()))
                .collect();
            if let Ok(parents) = parents {
                let time = i64::try_from(commit.committer_timestamp()).unwrap_or(i64::MAX);
                return Some((time, parents, commit.generation()));
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
            u32::MAX,
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
        let by_id: HashMap<ObjectId, (i64, super::Parents)> = commits
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

#[cfg(test)]
mod streamed_graph_order {
    use super::{ByTime, Horizon, Parents};
    use crate::topo::{in_graph_order, in_graph_order_streamed};
    use gix::ObjectId;
    use std::collections::HashMap;

    fn id(n: usize) -> ObjectId {
        let mut bytes = [0_u8; 20];
        bytes[..8].copy_from_slice(&(n as u64).to_be_bytes());
        bytes[8] = 0x5a;
        ObjectId::from_bytes_or_panic(&bytes)
    }

    fn number(oid: &ObjectId) -> usize {
        usize::try_from(u64::from_be_bytes(oid.as_bytes()[..8].try_into().unwrap())).unwrap()
    }

    struct Rng(u64);

    impl Rng {
        fn below(&mut self, n: usize) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            usize::try_from(self.0 >> 33).unwrap() % n.max(1)
        }
    }

    struct History {
        commits: HashMap<ObjectId, (i64, Parents)>,
        levels: HashMap<ObjectId, u32>,
        tips: Vec<ObjectId>,
        head: ObjectId,
    }

    fn history(seed: u64, size: usize) -> History {
        let mut rng = Rng(seed);
        let mut commits = HashMap::new();
        let mut every_level: HashMap<ObjectId, u32> = HashMap::new();
        let mut childless: Vec<bool> = Vec::new();
        let roots = 1 + rng.below(3);
        for n in 0..size {
            let mut parents = Parents::new();
            if n >= roots {
                let count = match rng.below(10) {
                    0..=5 => 1,
                    6..=8 => 2,
                    _ => 3,
                };
                for _ in 0..count {
                    let far = rng.below(4) == 0;
                    let reach = 1 + rng.below(if far { n } else { 6.min(n) });
                    let parent = id(n - reach);
                    if !parents.contains(&parent) {
                        parents.push(parent);
                    }
                }
            }
            let skew = if rng.below(8) == 0 {
                -i64::try_from(rng.below(40)).unwrap()
            } else {
                0
            };
            let time = i64::try_from(n * 3 / 2 + rng.below(2)).unwrap() + skew;
            let level = 1 + parents
                .iter()
                .map(|parent| every_level[parent])
                .max()
                .unwrap_or(0);
            for parent in &parents {
                childless[number(parent)] = false;
            }
            childless.push(true);
            every_level.insert(id(n), level);
            commits.insert(id(n), (time, parents));
        }
        let in_graph = match rng.below(3) {
            0 => size,
            1 => 0,
            _ => rng.below(size),
        };
        let levels = (0..in_graph)
            .map(|n| (id(n), every_level[&id(n)]))
            .collect();
        let mut tips: Vec<ObjectId> = (0..size).filter(|n| childless[*n]).map(id).collect();
        for _ in 0..rng.below(4) {
            tips.push(id(rng.below(size)));
        }
        let head = tips[rng.below(tips.len())];
        History {
            commits,
            levels,
            tips,
            head,
        }
    }

    fn order(history: &History, first_parent: bool, streamed: bool) -> Vec<ObjectId> {
        let read = |oid: ObjectId| history.commits.get(&oid).cloned();
        let horizon = Horizon::default();
        let level = |oid: &ObjectId| history.levels.get(oid).copied();
        let walk = ByTime::leveled(
            history.tips.clone(),
            read,
            streamed.then(|| (level, horizon.clone())),
            first_parent,
            Vec::new(),
        );
        if streamed {
            in_graph_order_streamed(walk, Some(history.head), first_parent, |oid| {
                horizon.settles(history.levels.get(oid).copied())
            })
            .map(|(oid, ..)| oid)
            .collect()
        } else {
            in_graph_order(walk, Some(history.head), first_parent)
                .map(|(oid, ..)| oid)
                .collect()
        }
    }

    #[test]
    fn the_streamed_order_is_the_order_of_the_whole_walk() {
        for seed in 0..600 {
            let history = history(seed, 3 + usize::try_from(seed % 70).unwrap());
            for first_parent in [false, true] {
                assert_eq!(
                    order(&history, first_parent, true),
                    order(&history, first_parent, false),
                    "seed {seed}, first parent {first_parent}"
                );
            }
        }
    }

    #[test]
    fn a_long_line_streams_with_levels() {
        let size = 5_000;
        let commits: HashMap<ObjectId, (i64, Parents)> = (0..size)
            .map(|n| {
                let parents = if n == 0 {
                    Parents::new()
                } else {
                    Parents::from_slice(&[id(n - 1)])
                };
                (id(n), (i64::try_from(n).unwrap(), parents))
            })
            .collect();
        let levels: HashMap<ObjectId, u32> = (0..size)
            .map(|n| (id(n), u32::try_from(n).unwrap() + 1))
            .collect();
        let reads = std::cell::Cell::new(0_usize);
        let read = |oid: ObjectId| {
            reads.set(reads.get() + 1);
            commits.get(&oid).cloned()
        };
        let horizon = Horizon::default();
        let walk = ByTime::leveled(
            [id(size - 1)],
            read,
            Some((|oid: &ObjectId| levels.get(oid).copied(), horizon.clone())),
            false,
            Vec::new(),
        );
        let first: Vec<ObjectId> =
            in_graph_order_streamed(walk, Some(id(size - 1)), false, |oid| {
                horizon.settles(levels.get(oid).copied())
            })
            .take(10)
            .map(|(oid, ..)| oid)
            .collect();
        assert_eq!(first, (size - 10..size).rev().map(id).collect::<Vec<_>>());
        assert!(reads.get() < 20, "read {}", reads.get());
    }
}
