/** Virtual folders over the open repositories. They group nothing on disk — a repository
    belongs to at most one, and one that belongs to none falls into the ungrouped bucket. */
export interface RepoGroups {
  order: string[];
  names: Record<string, string>;
  /** Repository root to group id. A root absent here is ungrouped. */
  of: Record<string, string>;
  /** Group id to the group it sits inside. Absent means it sits at the top. */
  under: Record<string, string>;
}

/** Not a group: the heading the unclaimed repositories gather under. */
export const UNGROUPED = "";

export type GroupRow =
  | { kind: "group"; id: string; name: string; count: number; depth: number }
  | { kind: "repo"; root: string; group: string; depth: number };

export function addGroup(
  groups: RepoGroups,
  name: string,
): { groups: RepoGroups; id: string | null } {
  const trimmed = name.trim();
  if (trimmed === "") return { groups, id: null };

  let id = `g${groups.order.length + 1}`;
  while (groups.names[id] !== undefined) id += "x";

  return {
    groups: {
      order: [...groups.order, id],
      names: { ...groups.names, [id]: trimmed },
      of: { ...groups.of },
      under: { ...groups.under },
    },
    id,
  };
}

export function renameGroup(groups: RepoGroups, id: string, name: string): RepoGroups {
  const trimmed = name.trim();
  if (trimmed === "" || groups.names[id] === undefined) return groups;
  return { ...groups, names: { ...groups.names, [id]: trimmed } };
}

/** The group goes; its repositories return to the ungrouped bucket rather than vanish. */
export function removeGroup(groups: RepoGroups, id: string): RepoGroups {
  const names = { ...groups.names };
  delete names[id];

  const of: Record<string, string> = {};
  for (const [root, group] of Object.entries(groups.of)) {
    if (group !== id) of[root] = group;
  }

  // Children move up to where their parent was rather than disappearing with it.
  const under: Record<string, string> = {};
  for (const [child, parent] of Object.entries(groups.under)) {
    if (child === id) continue;
    if (parent === id) {
      const grandparent = groups.under[id];
      if (grandparent) under[child] = grandparent;
    } else {
      under[child] = parent;
    }
  }

  return { order: groups.order.filter((entry) => entry !== id), names, of, under };
}

/** Moves a group inside another, or to the top level with `null`. A move that would make
    a group its own ancestor is refused: both ends would drop out of the tree. */
export function nest(groups: RepoGroups, id: string, parent: string | null): RepoGroups {
  if (groups.names[id] === undefined) return groups;

  if (parent === null) {
    if (groups.under[id] === undefined) return groups;
    const under = { ...groups.under };
    delete under[id];
    return { ...groups, under };
  }

  if (parent === id || groups.names[parent] === undefined) return groups;
  if (ancestors(groups, parent).includes(id)) return groups;
  return { ...groups, under: { ...groups.under, [id]: parent } };
}

function ancestors(groups: RepoGroups, id: string): string[] {
  const seen: string[] = [];
  let at: string | undefined = id;
  while (at !== undefined && !seen.includes(at)) {
    seen.push(at);
    at = groups.under[at];
  }
  return seen;
}

export function assign(groups: RepoGroups, root: string, group: string): RepoGroups {
  if (group !== UNGROUPED && groups.names[group] === undefined) return groups;

  const of = { ...groups.of };
  if (group === UNGROUPED) delete of[root];
  else of[root] = group;
  return { ...groups, of };
}

/** Headings and rows in one flat list, the way the panel draws them. */
export function groupRows(
  groups: RepoGroups,
  roots: readonly string[],
  collapsed: ReadonlySet<string>,
): GroupRow[] {
  if (groups.order.length === 0) {
    return roots.map((root) => ({ kind: "repo", root, group: UNGROUPED, depth: 0 }));
  }

  const rows: GroupRow[] = [];
  const claimed = new Set<string>();

  // Bucketed once. Walking every group and filtering every root per level made this
  // quadratic in the number of groups for no reason.
  const inGroup = new Map<string, string[]>();
  for (const root of roots) {
    const id = groups.of[root];
    if (id === undefined) continue;
    const bucket = inGroup.get(id);
    if (bucket) bucket.push(root);
    else inGroup.set(id, [root]);
  }

  const children = new Map<string | null, string[]>();
  for (const id of groups.order) {
    const parent = groups.under[id] ?? null;
    const bucket = children.get(parent);
    if (bucket) bucket.push(id);
    else children.set(parent, [id]);
  }

  // A collapsed group is still walked: its children's repositories belong to them, not to
  // Ungrouped. Collapsing only decides what is drawn.
  const walk = (parent: string | null, depth: number, shown: boolean) => {
    for (const id of children.get(parent) ?? []) {
      const inside = inGroup.get(id) ?? [];
      for (const root of inside) claimed.add(root);
      const open = shown && !collapsed.has(id);
      if (shown) {
        rows.push({
          kind: "group",
          id,
          name: groups.names[id] ?? id,
          count: inside.length,
          depth,
        });
      }
      if (open) {
        for (const root of inside) rows.push({ kind: "repo", root, group: id, depth: depth + 1 });
      }
      walk(id, depth + 1, open);
    }
  };
  walk(null, 0, true);

  const loose = roots.filter((root) => !claimed.has(root));
  if (loose.length === 0) return rows;

  rows.push({ kind: "group", id: UNGROUPED, name: "Ungrouped", count: loose.length, depth: 0 });
  if (!collapsed.has(UNGROUPED)) {
    for (const root of loose) rows.push({ kind: "repo", root, group: UNGROUPED, depth: 1 });
  }
  return rows;
}

function strings(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((entry) => typeof entry === "string") : [];
}

export function mergeGroups(stored: unknown): RepoGroups {
  const empty: RepoGroups = { order: [], names: {}, of: {}, under: {} };
  if (typeof stored !== "object" || stored === null) return empty;

  const source = stored as {
    order?: unknown;
    names?: unknown;
    of?: unknown;
    under?: unknown;
  };
  const names: Record<string, string> = {};
  if (typeof source.names === "object" && source.names !== null) {
    for (const [id, name] of Object.entries(source.names as Record<string, unknown>)) {
      if (typeof name === "string") names[id] = name;
    }
  }

  const order = strings(source.order).filter((id) => names[id] !== undefined);
  const of: Record<string, string> = {};
  if (typeof source.of === "object" && source.of !== null) {
    for (const [root, id] of Object.entries(source.of as Record<string, unknown>)) {
      // An assignment to a group that is gone would hide the repository entirely.
      if (typeof id === "string" && order.includes(id)) of[root] = id;
    }
  }
  const under: Record<string, string> = {};
  if (typeof source.under === "object" && source.under !== null) {
    for (const [child, parent] of Object.entries(source.under as Record<string, unknown>)) {
      if (typeof parent !== "string") continue;
      if (!order.includes(child) || !order.includes(parent) || child === parent) continue;
      under[child] = parent;
    }
  }
  // A stored cycle would leave every group in it unreachable from the top level.
  for (const child of Object.keys(under)) {
    let at: string | undefined = under[child];
    const seen = new Set([child]);
    while (at !== undefined && !seen.has(at)) {
      seen.add(at);
      at = under[at];
    }
    if (at !== undefined) delete under[child];
  }

  return { order, names, of, under };
}
