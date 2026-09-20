/** Virtual folders over the open repositories. They group nothing on disk — a repository
    belongs to at most one, and one that belongs to none falls into the ungrouped bucket. */
export interface RepoGroups {
  order: string[];
  names: Record<string, string>;
  /** Repository root to group id. A root absent here is ungrouped. */
  of: Record<string, string>;
}

/** Not a group: the heading the unclaimed repositories gather under. */
export const UNGROUPED = "";

export type GroupRow =
  | { kind: "group"; id: string; name: string; count: number }
  | { kind: "repo"; root: string; group: string };

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
  return { order: groups.order.filter((entry) => entry !== id), names, of };
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
    return roots.map((root) => ({ kind: "repo", root, group: UNGROUPED }));
  }

  const rows: GroupRow[] = [];
  const claimed = new Set<string>();

  for (const id of groups.order) {
    const inside = roots.filter((root) => groups.of[root] === id);
    for (const root of inside) claimed.add(root);
    rows.push({ kind: "group", id, name: groups.names[id] ?? id, count: inside.length });
    if (collapsed.has(id)) continue;
    for (const root of inside) rows.push({ kind: "repo", root, group: id });
  }

  const loose = roots.filter((root) => !claimed.has(root));
  if (loose.length === 0) return rows;

  rows.push({ kind: "group", id: UNGROUPED, name: "Ungrouped", count: loose.length });
  if (!collapsed.has(UNGROUPED)) {
    for (const root of loose) rows.push({ kind: "repo", root, group: UNGROUPED });
  }
  return rows;
}

function strings(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((entry) => typeof entry === "string") : [];
}

export function mergeGroups(stored: unknown): RepoGroups {
  const empty: RepoGroups = { order: [], names: {}, of: {} };
  if (typeof stored !== "object" || stored === null) return empty;

  const source = stored as { order?: unknown; names?: unknown; of?: unknown };
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
  return { order, names, of };
}
