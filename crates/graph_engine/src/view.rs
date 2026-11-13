use std::borrow::Borrow;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::Hash;

use serde::{Deserialize, Serialize};

/// A merge shown as one row, and how many commits its fold holds so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Fold {
    pub row: u32,
    pub hidden: u32,
}

/// Collapsed merges (#26); first parents only is the walk's own (R-341).
#[derive(Debug, Clone)]
pub struct ViewFilter<I = String> {
    collapse: bool,
    expanded: HashSet<I>,
    roots: HashSet<I>,
    wanted: HashSet<I>,
    folded: HashMap<I, u32>,
    hidden: HashMap<u32, u32>,
    grown: BTreeSet<u32>,
    rows: u32,
}

impl<I> Default for ViewFilter<I> {
    fn default() -> Self {
        Self {
            collapse: false,
            expanded: HashSet::new(),
            roots: HashSet::new(),
            wanted: HashSet::new(),
            folded: HashMap::new(),
            hidden: HashMap::new(),
            grown: BTreeSet::new(),
            rows: 0,
        }
    }
}

impl<I: Clone + Eq + Hash> ViewFilter<I> {
    #[must_use]
    pub fn collapse_merged(
        roots: impl IntoIterator<Item = I>,
        expanded: impl IntoIterator<Item = I>,
    ) -> Self {
        Self {
            collapse: true,
            roots: roots.into_iter().collect(),
            expanded: expanded.into_iter().collect(),
            ..Self::default()
        }
    }

    /// Children come before parents in the walk, so every claim on `oid` is in by now.
    pub fn admit<Q>(&mut self, oid: &Q, parents: &mut crate::Parents<I>) -> bool
    where
        I: Borrow<Q>,
        Q: ?Sized + Eq + Hash,
    {
        let wanted = self.wanted.remove(oid);
        let root = self.roots.remove(oid);
        let fold = self.folded.remove(oid);
        if !(root || wanted) {
            if let Some(row) = fold {
                *self.hidden.entry(row).or_default() += 1;
                self.grown.insert(row);
                for parent in parents.iter() {
                    if !self.wanted.contains::<I>(parent) && !self.folded.contains_key::<I>(parent)
                    {
                        self.folded.insert(parent.clone(), row);
                    }
                }
            }
            return false;
        }
        let row = self.rows;
        self.rows += 1;
        let folds = self.collapse && !self.expanded.contains(oid);
        if folds && parents.len() > 1 {
            let merged: crate::Parents<I> = parents.drain(1..).collect();
            for parent in merged {
                // A line to a commit the graph shows anyway stays.
                if self.roots.contains::<I>(&parent) || self.wanted.contains::<I>(&parent) {
                    parents.push(parent);
                } else {
                    self.folded.entry(parent).or_insert(row);
                }
            }
        }
        self.wanted.extend(parents.iter().cloned());
        true
    }

    pub fn take_folds(&mut self) -> Vec<Fold> {
        std::mem::take(&mut self.grown)
            .into_iter()
            .map(|row| Fold {
                row,
                hidden: self.hidden.get(&row).copied().unwrap_or(0),
            })
            .collect()
    }
}
