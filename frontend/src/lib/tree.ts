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
