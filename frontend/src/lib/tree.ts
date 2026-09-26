export interface TreeNode {
  id: string;
  depth: number;
  /** Set on a node that owns rows below it; a leaf leaves it out. */
  children?: boolean;
}

export type Flattened<T extends TreeNode> = T & { open?: boolean };

export function flatten<T extends TreeNode>(
  nodes: readonly T[],
  collapsed: ReadonlySet<string>,
): Flattened<T>[] {
  const rows: Flattened<T>[] = [];
  // The depth below which everything is hidden, or null while nothing is hidden. Only the
  // outermost collapsed node sets it, or reopening an inner one would reveal a branch.
  let hiddenBelow: number | null = null;

  for (const node of nodes) {
    if (hiddenBelow !== null) {
      if (node.depth > hiddenBelow) continue;
      hiddenBelow = null;
    }
    const open = node.children ? !collapsed.has(node.id) : undefined;
    rows.push(open === undefined ? { ...node } : { ...node, open });
    if (open === false) hiddenBelow = node.depth;
  }
  return rows;
}

export function subtree<T extends TreeNode>(nodes: readonly T[], id: string): T[] {
  const at = nodes.findIndex((node) => node.id === id);
  if (at < 0) return [];
  const root = nodes[at];
  if (!root) return [];

  const under: T[] = [];
  for (const node of nodes.slice(at + 1)) {
    if (node.depth <= root.depth) break;
    under.push(node);
  }
  return under;
}

export function toggle(collapsed: ReadonlySet<string>, id: string): Set<string> {
  const next = new Set(collapsed);
  if (!next.delete(id)) next.add(id);
  return next;
}

/** Folds made while a filter is typed, kept apart from the stored ones (R-485). */
export interface FilterFolds {
  filter: string;
  ids: ReadonlySet<string>;
}

export const NO_FILTER_FOLDS: FilterFolds = { filter: "", ids: new Set() };

/** A filter opens every node with a match: a match folded away is a match not found. A
    fold made under a filter lasts as long as its text, and clearing the filter brings
    back the folds from before, untouched. */
export function shownFolds(
  stored: ReadonlySet<string>,
  filter: string,
  folds: FilterFolds,
): ReadonlySet<string> {
  const text = filter.trim();
  if (text === "") return stored;
  return folds.filter === text ? folds.ids : NO_FILTER_FOLDS.ids;
}

export function toggleFilterFold(folds: FilterFolds, filter: string, id: string): FilterFolds {
  const text = filter.trim();
  return { filter: text, ids: toggle(folds.filter === text ? folds.ids : new Set(), id) };
}
