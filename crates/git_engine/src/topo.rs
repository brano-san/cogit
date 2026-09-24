//! `git log --date-order`: newest first, never a parent above a child (R-162). Arrival
//! is date order, so Kahn's window only holds back a commit whose child is still to come.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::hash::Hash;

/// Read ahead: a clock skewed by fewer commits than this is put right.
pub(crate) const LOOKAHEAD: usize = 2048;

/// What the order needs of a walked commit; the rest rides along.
pub(crate) trait Walked<Id> {
    fn id(&self) -> Id;
    fn parents(&self) -> &[Id];
}

impl<Id: Copy> Walked<Id> for (Id, Vec<Id>) {
    fn id(&self) -> Id {
        self.0
    }

    fn parents(&self) -> &[Id] {
        &self.1
    }
}

/// With the commit time the walk read, so a row needs no second read for it.
impl<Id: Copy> Walked<Id> for (Id, Vec<Id>, i64) {
    fn id(&self) -> Id {
        self.0
    }

    fn parents(&self) -> &[Id] {
        &self.1
    }
}

pub(crate) struct Ordered<Id, T, I> {
    source: I,
    drained: bool,
    lookahead: usize,
    arrived: u64,
    window: HashMap<Id, (u64, T)>,
    /// Unemitted children each commit still waits for; may exist before the commit.
    waiting: HashMap<Id, usize>,
    /// Earliest arrival first; an entry that gained a child to wait for is skipped.
    ready: BinaryHeap<Reverse<(u64, Id)>>,
    /// Goes out as soon as its children have: the commit HEAD is on.
    first: Option<Id>,
    first_arrived: bool,
}

/// `source` is newest first by commit time; `first` must be in it, or all of it is read first.
pub(crate) fn in_date_order<Id, T, I>(
    source: I,
    lookahead: usize,
    first: Option<Id>,
) -> Ordered<Id, T, I>
where
    Id: Copy + Eq + Ord + Hash,
    T: Walked<Id>,
    I: Iterator<Item = T>,
{
    Ordered {
        source,
        drained: false,
        lookahead: lookahead.max(1),
        arrived: 0,
        window: HashMap::new(),
        waiting: HashMap::new(),
        ready: BinaryHeap::new(),
        first,
        first_arrived: false,
    }
}

impl<Id, T, I> Ordered<Id, T, I>
where
    Id: Copy + Eq + Ord + Hash,
    T: Walked<Id>,
    I: Iterator<Item = T>,
{
    fn fill(&mut self) {
        while !self.drained
            && (self.window.len() < self.lookahead || self.first.is_some() && !self.first_arrived)
        {
            let Some(item) = self.source.next() else {
                self.drained = true;
                return;
            };
            let id = item.id();
            for parent in item.parents() {
                *self.waiting.entry(*parent).or_insert(0) += 1;
            }
            self.first_arrived |= self.first == Some(id);
            let seq = self.arrived;
            self.arrived += 1;
            if self.waiting.get(&id).copied().unwrap_or(0) == 0 {
                self.ready.push(Reverse((seq, id)));
            }
            self.window.insert(id, (seq, item));
        }
    }

    fn is_ready(&self, id: &Id) -> bool {
        self.window.contains_key(id) && self.waiting.get(id).copied().unwrap_or(0) == 0
    }

    fn emit(&mut self, id: Id) -> Option<T> {
        let (_, item) = self.window.remove(&id)?;
        for parent in item.parents() {
            let Some(count) = self.waiting.get_mut(parent) else {
                continue;
            };
            *count = count.saturating_sub(1);
            if *count == 0
                && let Some((seq, _)) = self.window.get(parent)
            {
                self.ready.push(Reverse((*seq, *parent)));
            }
        }
        Some(item)
    }

    /// Unreachable on an acyclic history; loud and deterministic rather than lossy.
    fn unblock(&mut self) {
        tracing::error!(
            stranded = self.window.len(),
            "commit order stalled; falling back to date order for the rest"
        );
        for (id, (seq, _)) in &self.window {
            self.waiting.remove(id);
            self.ready.push(Reverse((*seq, *id)));
        }
    }
}

impl<Id, T, I> Iterator for Ordered<Id, T, I>
where
    Id: Copy + Eq + Ord + Hash,
    T: Walked<Id>,
    I: Iterator<Item = T>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            self.fill();
            if let Some(first) = self.first.filter(|first| self.is_ready(first)) {
                self.first = None;
                if let Some(emitted) = self.emit(first) {
                    return Some(emitted);
                }
            }
            let Some(Reverse((_, id))) = self.ready.pop() else {
                if self.window.is_empty() {
                    return None;
                }
                self.unblock();
                continue;
            };
            if !self.is_ready(&id) {
                continue;
            }
            if let Some(emitted) = self.emit(id) {
                return Some(emitted);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::in_date_order as order;
    use std::collections::HashSet;

    fn run(commits: &[(u32, &[u32])], lookahead: usize) -> Vec<u32> {
        run_first(commits, lookahead, None)
    }

    fn run_first(commits: &[(u32, &[u32])], lookahead: usize, first: Option<u32>) -> Vec<u32> {
        let source = commits
            .iter()
            .map(|(id, parents)| (*id, parents.to_vec()))
            .collect::<Vec<_>>();
        order(source.into_iter(), lookahead, first)
            .map(|(id, _)| id)
            .collect()
    }

    fn is_topological(order: &[u32], commits: &[(u32, &[u32])]) -> bool {
        let position = |id: u32| order.iter().position(|seen| *seen == id);
        commits.iter().all(|(id, parents)| {
            parents
                .iter()
                .all(|parent| match (position(*id), position(*parent)) {
                    (Some(child), Some(parent)) => child < parent,
                    _ => true,
                })
        })
    }

    /// main: 8 -- 6 -- 4 -- 2 -- 1, side: 7 -- 5 -- 3, merged by 8. The id is the date.
    const INTERLEAVED: &[(u32, &[u32])] = &[
        (8, &[6, 7]),
        (7, &[5]),
        (6, &[4]),
        (5, &[3]),
        (4, &[2]),
        (3, &[2]),
        (2, &[1]),
        (1, &[]),
    ];

    #[test]
    fn a_straight_line_comes_out_untouched() {
        let line: &[(u32, &[u32])] = &[(4, &[3]), (3, &[2]), (2, &[1]), (1, &[])];
        assert_eq!(run(line, 16), vec![4, 3, 2, 1]);
    }

    /// Lines that lived side by side stay side by side: every edge as short as it can be.
    #[test]
    fn lines_that_lived_at_the_same_time_stay_in_date_order() {
        assert_eq!(run(INTERLEAVED, 16), vec![8, 7, 6, 5, 4, 3, 2, 1]);
    }

    #[test]
    fn a_parent_dated_after_its_child_still_comes_after_it() {
        let skewed: &[(u32, &[u32])] = &[(4, &[]), (3, &[4]), (2, &[3]), (1, &[])];
        let order = run(skewed, 16);
        assert_eq!(order, vec![2, 3, 4, 1]);
        assert!(is_topological(&order, skewed));
    }

    #[test]
    fn a_merge_is_never_emitted_after_the_branch_it_joins() {
        let order = run(INTERLEAVED, 16);
        assert!(is_topological(&order, INTERLEAVED), "{order:?}");
    }

    #[test]
    fn every_commit_is_emitted_exactly_once() {
        let skewed: &[(u32, &[u32])] = &[(4, &[]), (3, &[4]), (2, &[3]), (1, &[])];
        for lookahead in [1, 2, 3, 8, 64] {
            for commits in [INTERLEAVED, skewed] {
                let order = run(commits, lookahead);
                let unique: HashSet<u32> = order.iter().copied().collect();
                assert_eq!(unique.len(), commits.len(), "lookahead {lookahead}");
                assert_eq!(order.len(), commits.len(), "lookahead {lookahead}");
            }
        }
    }

    #[test]
    fn an_octopus_merge_comes_out_in_date_order() {
        let octopus: &[(u32, &[u32])] = &[
            (9, &[8, 6, 4]),
            (8, &[7]),
            (7, &[2]),
            (6, &[5]),
            (5, &[2]),
            (4, &[3]),
            (3, &[2]),
            (2, &[1]),
            (1, &[]),
        ];
        let order = run(octopus, 16);
        assert_eq!(order, vec![9, 8, 7, 6, 5, 4, 3, 2, 1]);
        assert!(is_topological(&order, octopus));
    }

    #[test]
    fn several_tips_come_out_newest_first() {
        let forest: &[(u32, &[u32])] = &[(4, &[2]), (3, &[1]), (2, &[]), (1, &[])];
        assert_eq!(run(forest, 16), vec![4, 3, 2, 1]);
    }

    #[test]
    fn the_commit_asked_for_first_comes_first_when_nothing_above_it_waits() {
        let two: &[(u32, &[u32])] = &[(5, &[3]), (4, &[2]), (3, &[1]), (2, &[1]), (1, &[])];
        assert_eq!(run_first(two, 16, Some(4)), vec![4, 5, 3, 2, 1]);
    }

    #[test]
    fn the_commit_asked_for_first_still_comes_after_its_children() {
        let behind: &[(u32, &[u32])] = &[(5, &[3]), (4, &[1]), (3, &[1]), (1, &[])];
        let order = run_first(behind, 16, Some(3));
        assert_eq!(order, vec![5, 3, 4, 1]);
        assert!(is_topological(&order, behind));
    }

    #[test]
    fn the_commit_asked_for_first_comes_first_however_far_down_its_date_puts_it() {
        let far: &[(u32, &[u32])] = &[(9, &[8]), (8, &[7]), (7, &[]), (2, &[1]), (1, &[])];
        assert_eq!(run_first(far, 2, Some(2)), vec![2, 9, 8, 7, 1]);
    }

    #[test]
    fn an_empty_stream_ends_at_once() {
        assert!(run(&[], 16).is_empty());
    }
}
