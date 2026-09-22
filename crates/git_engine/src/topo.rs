//! Re-orders a date-ordered commit stream so that each line of history comes out in one
//! piece, the way `git log --topo-order` reads.
//!
//! `gix`'s own topological walk counts every commit's children before it may emit the
//! first one, which costs a pass over the whole repository — 460 ms on the 50k fixture,
//! against a 300 ms budget for the first screen. This does the same bookkeeping inside a
//! sliding window instead, so the cost is paid per screen rather than per repository
//! (doc/12-risks.md, R-140).

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

/// Commits read ahead before the first one is emitted. A line of history longer than this
/// falls back to date order for its tail, which is the same thing the user sees today.
pub(crate) const LOOKAHEAD: usize = 2048;

/// Kahn's algorithm over a window of the stream. Depth-first, so emitting a merge walks
/// the branch it joined before returning to the line the merge sits on.
pub(crate) struct Grouped<Id, I> {
    source: I,
    drained: bool,
    lookahead: usize,
    /// Read, not yet emitted, with the parents still to visit.
    window: HashMap<Id, Vec<Id>>,
    /// Unemitted children each commit is still waiting for. An entry may exist before the
    /// commit itself is read: in date order children always arrive first.
    waiting: HashMap<Id, usize>,
    /// Ready because a child was just emitted. A stack, and that is the whole trick —
    /// taking the newest ready commit instead is exactly the date order we are undoing.
    line: Vec<Id>,
    /// Ready on arrival — a tip. In the order the source gave them, so newest first.
    tips: VecDeque<Id>,
}

/// `source` yields `(id, parents)` newest first by commit time.
pub(crate) fn group_topologically<Id, I>(source: I, lookahead: usize) -> Grouped<Id, I>
where
    Id: Copy + Eq + Ord + Hash,
    I: Iterator<Item = (Id, Vec<Id>)>,
{
    Grouped {
        source,
        drained: false,
        lookahead: lookahead.max(1),
        window: HashMap::new(),
        waiting: HashMap::new(),
        line: Vec::new(),
        tips: VecDeque::new(),
    }
}

impl<Id, I> Grouped<Id, I>
where
    Id: Copy + Eq + Ord + Hash,
    I: Iterator<Item = (Id, Vec<Id>)>,
{
    fn fill(&mut self) {
        while !self.drained && self.window.len() < self.lookahead {
            let Some((id, parents)) = self.source.next() else {
                self.drained = true;
                return;
            };
            for parent in &parents {
                *self.waiting.entry(*parent).or_insert(0) += 1;
            }
            if self.waiting.get(&id).copied().unwrap_or(0) == 0 {
                self.tips.push_back(id);
            }
            self.window.insert(id, parents);
        }
    }

    /// Every non-empty window holds a commit with no unemitted children — follow any chain
    /// of children upwards and it ends, the history being finite and acyclic. Reaching the
    /// fallback means the bookkeeping is wrong, so it is loud and deterministic rather
    /// than silently dropping the rest of the history.
    fn unblock(&mut self) {
        let mut ready: Vec<Id> = self.window.keys().copied().collect();
        ready.sort_unstable();
        tracing::error!(
            stranded = ready.len(),
            "topological order stalled; falling back to date order for the rest"
        );
        self.tips.extend(ready);
    }
}

impl<Id, I> Iterator for Grouped<Id, I>
where
    Id: Copy + Eq + Ord + Hash,
    I: Iterator<Item = (Id, Vec<Id>)>,
{
    type Item = (Id, Vec<Id>);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            self.fill();
            if self.line.is_empty() && self.tips.is_empty() {
                if self.window.is_empty() {
                    return None;
                }
                self.unblock();
            }

            let id = self.line.pop().or_else(|| self.tips.pop_front())?;
            let Some(parents) = self.window.remove(&id) else {
                continue;
            };

            for parent in &parents {
                let Some(count) = self.waiting.get_mut(parent) else {
                    continue;
                };
                *count = count.saturating_sub(1);
                if *count == 0 && self.window.contains_key(parent) {
                    self.line.push(*parent);
                }
            }
            return Some((id, parents));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::group_topologically as group;
    use std::collections::HashSet;

    /// `(id, parents)` pairs, the way a date-ordered walk hands them over.
    fn run(commits: &[(u32, &[u32])], lookahead: usize) -> Vec<u32> {
        let source = commits
            .iter()
            .map(|(id, parents)| (*id, parents.to_vec()))
            .collect::<Vec<_>>();
        group(source.into_iter(), lookahead)
            .map(|(id, _)| id)
            .collect()
    }

    /// Ids count down, so a commit's parents always have smaller ids: a valid order is one
    /// where nothing appears after its parents.
    fn is_topological(order: &[u32], commits: &[(u32, &[u32])]) -> bool {
        let position = |id: u32| order.iter().position(|seen| *seen == id);
        commits.iter().all(|(id, parents)| {
            parents.iter().all(|parent| {
                match (position(*id), position(*parent)) {
                    (Some(child), Some(parent)) => child < parent,
                    _ => true, // a parent outside the stream is not our business
                }
            })
        })
    }

    /// main:  8 -- 6 -- 4 -- 2 -- 1     (8 is the merge)
    /// side:   \- 7 -- 5 -- 3 --/
    /// The id is also the commit date, so the date walk hands these over as 8..1 and the
    /// two lines arrive one row each in turn, which is what makes the graph unreadable.
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

    #[test]
    fn two_lines_of_history_stop_interleaving() {
        assert_eq!(run(INTERLEAVED, 16), vec![8, 7, 5, 3, 6, 4, 2, 1]);
    }

    #[test]
    fn a_merge_is_never_emitted_after_the_branch_it_joins() {
        let order = run(INTERLEAVED, 16);
        assert!(is_topological(&order, INTERLEAVED), "{order:?}");
    }

    #[test]
    fn every_commit_is_emitted_exactly_once() {
        for lookahead in [1, 2, 3, 8, 64] {
            let order = run(INTERLEAVED, lookahead);
            let unique: HashSet<u32> = order.iter().copied().collect();
            assert_eq!(unique.len(), INTERLEAVED.len(), "lookahead {lookahead}");
            assert_eq!(order.len(), INTERLEAVED.len(), "lookahead {lookahead}");
        }
    }

    #[test]
    fn a_lookahead_too_small_to_group_still_orders_topologically() {
        let order = run(INTERLEAVED, 1);
        assert!(is_topological(&order, INTERLEAVED), "{order:?}");
    }

    #[test]
    fn an_octopus_merge_keeps_each_of_its_branches_together() {
        // 9 merges three lines that were committed in alternation.
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
        assert!(is_topological(&order, octopus), "{order:?}");
        let run_of = |first: u32| order.windows(2).any(|pair| pair == [first, first - 1]);
        assert!(run_of(8) && run_of(6) && run_of(4), "{order:?}");
    }

    #[test]
    fn several_tips_are_started_newest_first() {
        // Two roots that never meet: the newer tip's line has to come out first.
        let forest: &[(u32, &[u32])] = &[(4, &[2]), (3, &[1]), (2, &[]), (1, &[])];
        assert_eq!(run(forest, 16), vec![4, 2, 3, 1]);
    }

    #[test]
    fn a_parent_dated_after_its_child_is_still_emitted_once() {
        // Clock skew: 3's parent 4 carries a newer date and arrives first.
        let skewed: &[(u32, &[u32])] = &[(4, &[]), (3, &[4]), (2, &[3]), (1, &[])];
        let order = run(skewed, 16);
        assert_eq!(order.len(), 4);
        assert_eq!(order.iter().collect::<HashSet<_>>().len(), 4, "{order:?}");
    }

    #[test]
    fn an_empty_stream_ends_at_once() {
        assert!(run(&[], 16).is_empty());
    }
}
