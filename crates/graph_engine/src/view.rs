use std::collections::{BTreeSet, HashMap, HashSet};

use serde::Serialize;

/// A merge shown as one row, and how many commits its fold holds so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Fold {
    pub row: u32,
    pub hidden: u32,
}

/// Collapsed merges (#26); first parents only is the walk's own (R-341).
#[derive(Debug, Clone, Default)]
pub struct ViewFilter {
    collapse: bool,
    expanded: HashSet<String>,
    roots: HashSet<String>,
    wanted: HashSet<String>,
    folded: HashMap<String, u32>,
    hidden: HashMap<u32, u32>,
    grown: BTreeSet<u32>,
    rows: u32,
}

impl ViewFilter {
    #[must_use]
    pub fn collapse_merged(
        roots: impl IntoIterator<Item = String>,
        expanded: impl IntoIterator<Item = String>,
    ) -> Self {
        Self {
            collapse: true,
            roots: roots.into_iter().collect(),
            expanded: expanded.into_iter().collect(),
            ..Self::default()
        }
    }

    /// Children come before parents in the walk, so every claim on `oid` is in by now.
    pub fn admit(&mut self, oid: &str, parents: &mut Vec<String>) -> bool {
        let wanted = self.wanted.remove(oid);
        let root = self.roots.remove(oid);
        let fold = self.folded.remove(oid);
        if !(root || wanted) {
            if let Some(row) = fold {
                *self.hidden.entry(row).or_default() += 1;
                self.grown.insert(row);
                for parent in parents.iter() {
                    if !self.wanted.contains(parent) && !self.folded.contains_key(parent) {
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
            let merged = parents.split_off(1);
            for parent in merged {
                // A line to a commit the graph shows anyway stays.
                if self.roots.contains(&parent) || self.wanted.contains(&parent) {
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
