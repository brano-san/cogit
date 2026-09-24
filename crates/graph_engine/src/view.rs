use std::collections::HashSet;

#[derive(Debug, Clone, Default)]
pub struct ViewFilter {
    roots: HashSet<String>,
    wanted: HashSet<String>,
}

impl ViewFilter {
    #[must_use]
    pub fn first_parent(roots: impl IntoIterator<Item = String>) -> Self {
        Self {
            roots: roots.into_iter().collect(),
            wanted: HashSet::new(),
        }
    }

    /// Children come before parents in the walk, so every claim on `oid` is in by now.
    pub fn admit(&mut self, oid: &str, parents: &mut Vec<String>) -> bool {
        let wanted = self.wanted.remove(oid);
        if !(self.roots.remove(oid) || wanted) {
            return false;
        }
        parents.truncate(1);
        self.wanted.extend(parents.iter().cloned());
        true
    }
}
