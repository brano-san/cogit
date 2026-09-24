//! The walk and its lane layout, chunk by chunk (R-51, R-161, R-162).

use crate::GraphChunk;

/// A filtered history is a flat list, not a graph: the parents of a match are usually
/// filtered out, so lanes drawn between survivors would claim a lineage that is not
/// there. Other clients do the same. Narrowing the visible refs is exempt — it drops
/// whole tips, never a commit from inside a surviving lineage (R-51).
pub(crate) fn lay_out(
    handle: &git_engine::RepoHandle,
    query: &git_engine::CommitQuery,
    chunk_size: usize,
    reuse: Option<git_engine::Reuse<'_>>,
    record: Option<&mut git_engine::WalkedHistory>,
    mut on_chunk: impl FnMut(GraphChunk) -> bool,
) -> Result<Vec<git_engine::SkippedRef>, git_engine::GitError> {
    let flat = query.filters_rows();

    // Read once per load: a column that moved half way down would be worse than none.
    let mut cursor = graph_engine::LayoutCursor::with_mainline(mainline_of(handle, query));
    let mut cancelled = false;

    // One order for the graph and the filtered list: by date, never a parent above a
    // child (R-162). A line to a parent the list will not show ends in an arrow (R-161).
    let on_commits = |commits: Vec<git_engine::CommitRow>| {
        let nodes: Vec<graph_engine::CommitNode> = commits
            .iter()
            .map(|c| graph_engine::CommitNode {
                oid: c.oid.clone(),
                parents: c.parents.clone(),
                hidden: if flat {
                    c.parents
                        .iter()
                        .filter(|parent| !handle.shown_by(query, parent))
                        .cloned()
                        .collect()
                } else {
                    Vec::new()
                },
            })
            .collect();
        let rows = graph_engine::layout(&nodes, &mut cursor);

        let keep = on_chunk(GraphChunk {
            commits,
            rows,
            is_last: false,
        });
        cancelled = !keep;
        keep
    };

    let skipped = handle.graph_commits(query, chunk_size, reuse, record, on_commits)?;

    if !cancelled {
        on_chunk(GraphChunk {
            commits: Vec::new(),
            rows: Vec::new(),
            is_last: true,
        });
    }
    Ok(skipped)
}

/// HEAD, then `master`, then `main` — of those the graph draws. A primary ref that is
/// unticked or filtered out would hold column 0 empty for a line that never comes (R-161).
fn mainline_of(handle: &git_engine::RepoHandle, query: &git_engine::CommitQuery) -> Option<String> {
    let ticked = |name: &str| {
        query
            .visible_refs
            .as_ref()
            .is_none_or(|refs| refs.iter().any(|rev| rev == name))
    };
    let branches = handle.branches().ok()?;
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
    (!query.filters_rows() || handle.shown_by(query, &tip)).then_some(tip)
}
