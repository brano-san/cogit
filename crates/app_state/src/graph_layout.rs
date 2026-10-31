//! The walk and its lane layout, chunk by chunk (R-51, R-161, R-162).

use crate::GraphChunk;

/// A filtered history is a flat list, not a graph: the parents of a match are usually
/// filtered out, so lanes drawn between survivors would claim a lineage that is not
/// there. Other clients do the same. Narrowing the visible refs is exempt — it drops
/// whole tips, never a commit from inside a surviving lineage (R-51). Show Graph While
/// Filtering draws the lines anyway, down first parents through what the filter left out
/// (F-561, R-575).
pub(crate) fn lay_out(
    handle: &git_engine::RepoHandle,
    query: &git_engine::CommitQuery,
    chunk_size: usize,
    rows: git_engine::GraphRows<'_, '_>,
    mut on_chunk: impl FnMut(GraphChunk) -> bool,
) -> Result<Vec<git_engine::SkippedRef>, git_engine::GitError> {
    let flat = query.filters_rows();
    let lines = flat && query.view.filtered_graph;
    let shown = (flat && !lines).then(|| handle.shown_filter(query));
    let passed = git_engine::PassedCommits::default();
    // The walk follows first parents itself (R-301); a merge shows its first line only (#26).
    let first_parent = query.view.first_parent && !flat;

    // Read once per load: a column that moved half way down would be worse than none.
    // A line passed through left-out commits is placed as they come, never held back.
    let long_links = if lines {
        0
    } else {
        query.long_link_rows.unwrap_or(0)
    };
    // The long-link window is also how far a new line's length is known (07 §3).
    let mut cursor = graph_engine::LayoutCursor::with_mainline(mainline_of(handle, query))
        .with_long_links(long_links);
    let mut cancelled = false;
    let mut view = graph_view(handle, query, flat)?;
    let mut tree = WorkingTree::of(handle, query, flat, first_parent);
    // A shallow boundary or an unreadable parent: a line to it ends in an arrow (07 §4).
    let cut = git_engine::CutParents::default();
    let carry = rows.text || flat;
    let rows = git_engine::GraphRows {
        cut: Some(&cut),
        ..rows
    };

    // One order for the graph and the filtered list: by date, never a parent above a
    // child (R-162). A line to a parent the list will not show ends in an arrow (R-161).
    let on_commits = |mut commits: Vec<git_engine::CommitRow>| {
        if first_parent {
            for commit in &mut commits {
                commit.parents.truncate(1);
            }
        }
        if let Some(view) = view.as_mut() {
            commits.retain_mut(|c| view.admit(&c.oid, &mut c.parents));
        }
        let laid: Vec<crate::graph_cache::Node> =
            commits.iter().map(crate::graph_cache::Node::of).collect();
        let texts: Vec<git_engine::CommitText> = if carry {
            commits
                .iter_mut()
                .map(crate::graph_cache::take_text)
                .collect()
        } else {
            Vec::new()
        };
        let mut nodes: Vec<graph_engine::CommitNode> = commits
            .into_iter()
            .map(|c| graph_engine::CommitNode {
                hidden: if let Some(shown) = &shown {
                    c.parents
                        .iter()
                        .filter(|parent| !shown.shows(handle, parent))
                        .cloned()
                        .collect()
                } else if cut.is_empty() {
                    Vec::new()
                } else {
                    c.parents
                        .iter()
                        .filter(|parent| cut.hides(&c.oid, parent))
                        .cloned()
                        .collect()
                },
                oid: c.oid,
                parents: c.parents,
            })
            .collect();
        if let Some(tree) = tree.as_mut() {
            tree.insert(&mut nodes);
        }
        let rows = if lines {
            let mut passes = passed.take().into_iter().peekable();
            let mut rows = Vec::with_capacity(nodes.len());
            for (at, node) in nodes.into_iter().enumerate() {
                while let Some(pass) = passes.next_if(|pass| pass.before <= at) {
                    graph_engine::pass_through(
                        &mut cursor,
                        &pass.oid,
                        pass.first_parent.as_deref(),
                    );
                }
                rows.extend(graph_engine::push(vec![node], &mut cursor));
            }
            for pass in passes {
                graph_engine::pass_through(&mut cursor, &pass.oid, pass.first_parent.as_deref());
            }
            rows
        } else {
            graph_engine::push(nodes, &mut cursor)
        };
        let folds = view
            .as_mut()
            .map(graph_engine::ViewFilter::take_folds)
            .unwrap_or_default();
        let mut rows = rows;
        let working_tree = tree.as_mut().and_then(|tree| tree.take(&mut rows));

        let keep = on_chunk(GraphChunk {
            commits: laid,
            texts,
            rows,
            folds,
            working_tree,
            is_last: false,
        });
        cancelled = !keep;
        keep
    };

    let skipped = handle.graph_commits_passing(
        query,
        chunk_size,
        rows,
        lines.then_some(&passed),
        on_commits,
    )?;

    if !cancelled {
        // The rows held back to see how far their links reach (R-330).
        let mut rows = graph_engine::finish(&mut cursor);
        let working_tree = tree.as_mut().and_then(|tree| tree.take(&mut rows));
        on_chunk(GraphChunk {
            commits: Vec::new(),
            texts: Vec::new(),
            rows,
            working_tree,
            folds: view
                .as_mut()
                .map(graph_engine::ViewFilter::take_folds)
                .unwrap_or_default(),
            is_last: true,
        });
    }
    Ok(skipped)
}

/// Collapsed merges (#26). First parents only is the walk's own business (R-301), and a
/// filtered list is flat already.
fn graph_view(
    handle: &git_engine::RepoHandle,
    query: &git_engine::CommitQuery,
    flat: bool,
) -> Result<Option<graph_engine::ViewFilter>, git_engine::GitError> {
    let view = &query.view;
    if flat || view.first_parent || !view.collapse_merged {
        return Ok(None);
    }
    let roots = handle.walk_tips(query)?;
    Ok(Some(graph_engine::ViewFilter::collapse_merged(
        roots,
        view.expanded.iter().cloned(),
    )))
}

/// HEAD, then `master`, then `main` — of those the graph draws: the line drawn as the main
/// one from there down. It picks no column; the top commit has column 0 (R-783).
fn mainline_of(handle: &git_engine::RepoHandle, query: &git_engine::CommitQuery) -> Option<String> {
    let ticked = |name: &str| {
        query
            .visible_refs
            .as_ref()
            .is_none_or(|refs| refs.iter().any(|rev| rev == name))
    };
    let branches = handle.branches_without_divergence().ok()?;
    let locals: Vec<(&str, &str)> = branches
        .iter()
        .filter(|branch| {
            branch.kind == git_engine::BranchKind::Local
                && ticked(&format!("refs/heads/{}", branch.name))
        })
        .map(|branch| (branch.name.as_str(), branch.oid.as_str()))
        .collect();
    // `head()`, not the branch marked as HEAD: a detached HEAD is a line worth keeping
    // straight too, and it belongs to no branch.
    let head = match handle.head().ok()? {
        git_engine::Head::Branch { oid, .. } | git_engine::Head::Detached { oid } => Some(oid),
        git_engine::Head::Unborn { .. } => None,
    }
    .filter(|_| ticked("HEAD"));
    let tip = graph_engine::mainline_tip(&locals, head.as_deref())?;
    // Lines through left-out commits keep the main line on HEAD's first parents all the way.
    (!query.filters_rows() || query.view.filtered_graph || handle.shown_by(query, &tip))
        .then_some(tip)
}

/// A merge stopped on conflicts is drawn as if it were done (07 §4): right above HEAD a node
/// for the working tree, with HEAD and MERGE_HEAD for parents, goes through the layout like
/// a commit. Its row is taken out of the list, which keeps one row per commit, and drawn as
/// the Working Tree row.
struct WorkingTree {
    head: String,
    parents: Vec<String>,
    /// Nodes pushed so far, then the row the node got.
    pushed: u32,
    row: Option<u32>,
}

impl WorkingTree {
    const OID: &str = "working-tree";

    fn of(
        handle: &git_engine::RepoHandle,
        query: &git_engine::CommitQuery,
        flat: bool,
        first_parent: bool,
    ) -> Option<Self> {
        let ticked = query
            .visible_refs
            .as_ref()
            .is_none_or(|refs| refs.iter().any(|rev| rev == "HEAD"));
        if flat || !ticked {
            return None;
        }
        let merge = handle.merge_head()?.to_string();
        let head = match handle.head().ok()? {
            git_engine::Head::Branch { oid, .. } | git_engine::Head::Detached { oid } => oid,
            git_engine::Head::Unborn { .. } => return None,
        };
        let mut parents = vec![head.clone(), merge];
        if first_parent {
            parents.truncate(1);
        }
        Some(Self {
            head,
            parents,
            pushed: 0,
            row: None,
        })
    }

    fn insert(&mut self, nodes: &mut Vec<graph_engine::CommitNode>) {
        let at = if self.row.is_none() {
            nodes.iter().position(|node| node.oid == self.head)
        } else {
            None
        };
        if let Some(at) = at {
            nodes.insert(
                at,
                graph_engine::CommitNode {
                    oid: Self::OID.to_owned(),
                    parents: self.parents.clone(),
                    hidden: Vec::new(),
                },
            );
            self.row = Some(self.pushed + u32::try_from(at).unwrap_or(u32::MAX));
        }
        self.pushed = self
            .pushed
            .saturating_add(u32::try_from(nodes.len()).unwrap_or(u32::MAX));
    }

    /// Its row out of `rows`, and the rows after it one up.
    fn take(&self, rows: &mut Vec<graph_engine::GraphRow>) -> Option<graph_engine::GraphRow> {
        let at = self.row?;
        let mut taken = None;
        rows.retain_mut(|row| {
            if row.row == at {
                row.kind = graph_engine::NodeKind::WorkingTree;
                taken = Some(row.clone());
                return false;
            }
            if row.row > at {
                row.row -= 1;
            }
            for link in &mut row.links {
                if link.oid == Self::OID {
                    link.oid.clone_from(&self.head);
                }
            }
            true
        });
        taken
    }
}
