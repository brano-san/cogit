use crate::graph_walk::Parents;
use gix::ObjectId;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

type Queue<K> = BinaryHeap<(K, Reverse<ObjectId>)>;

struct Node {
    time: i64,
    parents: Parents,
    level: u32,
    waiting: usize,
    expanded: bool,
}

pub(crate) struct SmartOrder<R> {
    read: R,
    first: Option<ObjectId>,
    first_parent: bool,
    shallow: Vec<ObjectId>,
    merge: Option<(ObjectId, ObjectId)>,
    nodes: gix::hashtable::HashMap<ObjectId, Node>,
    unreadable: gix::hashtable::HashSet<ObjectId>,
    frontier: Queue<(u32, i64)>,
    ready: Queue<i64>,
    preferred: Vec<ObjectId>,
}

impl<R: FnMut(ObjectId) -> Option<(i64, Parents, u32)>> SmartOrder<R> {
    pub(crate) fn new(
        tips: Vec<ObjectId>,
        read: R,
        first: Option<ObjectId>,
        first_parent: bool,
        shallow: Vec<ObjectId>,
        merge: Option<(ObjectId, ObjectId)>,
    ) -> Self {
        let mut walk = Self {
            read,
            first,
            first_parent,
            shallow,
            merge: None,
            nodes: Default::default(),
            unreadable: Default::default(),
            frontier: BinaryHeap::new(),
            ready: BinaryHeap::new(),
            preferred: Vec::new(),
        };
        for tip in tips {
            if let Some(node) = walk.discover(tip) {
                let time = node.time;
                walk.ready.push((time, Reverse(tip)));
            }
        }
        if let Some((head, merge)) = merge {
            walk.settle(0);
            if walk.nodes.contains_key(&head)
                && !walk.reaches(merge, head)
                && let Some(node) = walk.nodes.get_mut(&merge)
            {
                node.waiting += 1;
                walk.merge = Some((head, merge));
            }
        }
        walk
    }

    fn discover(&mut self, id: ObjectId) -> Option<&mut Node> {
        use gix::hashtable::hash_map::Entry;
        match self.nodes.entry(id) {
            Entry::Occupied(node) => Some(node.into_mut()),
            Entry::Vacant(slot) => {
                if self.unreadable.contains(&id) {
                    return None;
                }
                let Some((time, parents, level)) = (self.read)(id) else {
                    self.unreadable.insert(id);
                    return None;
                };
                self.frontier.push(((level, time), Reverse(id)));
                Some(slot.insert(Node {
                    time,
                    parents,
                    level,
                    waiting: 0,
                    expanded: false,
                }))
            }
        }
    }

    fn follow_count(&self, id: ObjectId) -> usize {
        if self.shallow.binary_search(&id).is_ok() {
            0
        } else if self.first_parent {
            1
        } else {
            usize::MAX
        }
    }

    fn expand(&mut self, id: ObjectId) {
        let follow = self.follow_count(id);
        let Some(node) = self.nodes.get_mut(&id).filter(|node| !node.expanded) else {
            return;
        };
        node.expanded = true;
        let parents: Parents = node.parents.iter().take(follow).copied().collect();
        for parent in parents {
            if let Some(node) = self.discover(parent) {
                node.waiting += 1;
            }
        }
    }

    fn settle(&mut self, level: u32) {
        while let Some(&((next, _), Reverse(id))) = self.frontier.peek() {
            if next != u32::MAX && next <= level {
                break;
            }
            self.frontier.pop();
            self.expand(id);
        }
    }

    fn reaches(&self, start: ObjectId, target: ObjectId) -> bool {
        let mut pending = vec![start];
        let mut seen = gix::hashtable::HashSet::default();
        while let Some(id) = pending.pop() {
            if id == target {
                return true;
            }
            if seen.insert(id)
                && let Some(node) = self.nodes.get(&id)
            {
                pending.extend(node.parents.iter().take(self.follow_count(id)).copied());
            }
        }
        false
    }

    fn is_ready(&mut self, id: ObjectId) -> bool {
        let Some(node) = self.nodes.get(&id).filter(|node| node.waiting == 0) else {
            return false;
        };
        self.settle(node.level);
        self.nodes.get(&id).is_some_and(|node| node.waiting == 0)
    }

    fn release(&mut self, id: ObjectId, prefer: bool) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.waiting -= 1;
            if node.waiting == 0 {
                if prefer {
                    self.preferred.push(id);
                } else {
                    self.ready.push((node.time, Reverse(id)));
                }
            }
        }
    }
}

impl<R: FnMut(ObjectId) -> Option<(i64, Parents, u32)>> Iterator for SmartOrder<R> {
    type Item = (ObjectId, Parents, i64);

    fn next(&mut self) -> Option<Self::Item> {
        let (id, on_side) = loop {
            if let Some(id) = self.first.take() {
                if self.is_ready(id) {
                    break (id, false);
                }
            } else if let Some(id) = self.preferred.pop() {
                if self.is_ready(id) {
                    break (id, true);
                }
            } else {
                let (_, Reverse(id)) = self.ready.pop()?;
                if self.is_ready(id) {
                    break (id, false);
                }
            }
        };
        if self
            .frontier
            .peek()
            .is_some_and(|&(_, Reverse(top))| top == id)
        {
            self.frontier.pop();
        }
        self.expand(id);
        let node = self.nodes.remove(&id)?;
        if let Some((_, merge)) = self.merge.filter(|(head, _)| *head == id) {
            self.merge = None;
            self.release(merge, true);
        }
        let count = node.parents.len().min(self.follow_count(id));
        if let Some((first, side)) = node.parents[..count].split_first() {
            self.release(*first, on_side);
            for parent in side.iter().rev() {
                self.release(*parent, true);
            }
        }
        Some((id, node.parents, node.time))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u8) -> ObjectId {
        ObjectId::from_bytes_or_panic(&[n; 20])
    }

    fn ordered(
        tips: &[u8],
        commits: &[(u8, i64, u32, &[u8])],
        first_parent: bool,
        indexed: bool,
    ) -> Vec<ObjectId> {
        SmartOrder::new(
            tips.iter().copied().map(id).collect(),
            |oid| {
                let &(_, time, level, parents) = commits.iter().find(|row| id(row.0) == oid)?;
                let level = if indexed { level } else { u32::MAX };
                Some((time, parents.iter().copied().map(id).collect(), level))
            },
            None,
            first_parent,
            Vec::new(),
            None,
        )
        .map(|(oid, ..)| oid)
        .collect()
    }

    const OCTOPUS: &[(u8, i64, u32, &[u8])] = &[
        (1, 0, 1, &[]),
        (2, 80, 2, &[1]),
        (3, 95, 2, &[1]),
        (4, 10, 3, &[3]),
        (5, 35, 2, &[1]),
        (6, 20, 3, &[5]),
        (7, 100, 4, &[2, 4, 6]),
        (8, 90, 1, &[]),
        (9, 90, 1, &[]),
    ];

    #[test]
    fn an_octopus_keeps_each_side_contiguous_then_resumes_by_time() {
        let expected = [7, 4, 3, 6, 5, 8, 9, 2, 1].map(id);
        for indexed in [false, true] {
            assert_eq!(ordered(&[9, 3, 8, 7], OCTOPUS, false, indexed), expected);
        }
    }

    #[test]
    fn first_parent_skips_merge_edges_and_respects_separately_selected_tips() {
        let expected = [7, 3, 8, 9, 2, 1].map(id);
        for indexed in [false, true] {
            assert_eq!(ordered(&[9, 3, 8, 7], OCTOPUS, true, indexed), expected);
        }
    }

    #[test]
    fn a_newer_indexed_tip_waits_for_its_unindexed_child() {
        let commits: &[(u8, i64, u32, &[u8])] =
            &[(1, 100, 1, &[]), (2, 10, u32::MAX, &[1]), (3, 50, 1, &[])];
        let expected = [3, 2, 1].map(id);
        for indexed in [false, true] {
            assert_eq!(ordered(&[1, 2, 3], commits, false, indexed), expected);
        }
    }

    #[test]
    fn a_long_indexed_history_stops_reading_when_the_caller_stops() {
        let oid = |n: u32| {
            let mut bytes = [0; 20];
            bytes[..4].copy_from_slice(&n.to_be_bytes());
            ObjectId::from_bytes_or_panic(&bytes)
        };
        let reads = std::cell::Cell::new(0);
        let walk = SmartOrder::new(
            vec![oid(50_000)],
            |id| {
                reads.set(reads.get() + 1);
                let n = u32::from_be_bytes(id.as_bytes()[..4].try_into().ok()?);
                let parents = (n > 1).then(|| oid(n - 1)).into_iter().collect();
                Some((i64::from(n), parents, n))
            },
            Some(oid(50_000)),
            false,
            Vec::new(),
            None,
        );
        let first: Vec<_> = walk.take(200).map(|(id, ..)| id).collect();
        assert_eq!(first, (49_801..=50_000).rev().map(oid).collect::<Vec<_>>());
        assert_eq!(reads.get(), 201);
    }
}
