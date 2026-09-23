import type { RepoOverview } from "./ipc";

/** What the Repositories list remembers beyond what is open: repositories closed but kept
    in the list, the names the user gave them, and which are pinned (#36). Keyed by root,
    as the groups are; nothing here touches the folder or the repository itself. */
export interface RepoList {
  closed: string[];
  names: Record<string, string>;
  /** In the order they were pinned: the first pinned stays on top. */
  pinned: string[];
}

export const EMPTY_LIST: RepoList = { closed: [], names: {}, pinned: [] };

/** One row of the list, open or closed. */
export interface ListedRepo {
  root: string;
  name: string;
  /** `null` while the repository is closed. */
  overview: RepoOverview | null;
  pinned: boolean;
}

function strings(value: unknown): string[] {
  return Array.isArray(value)
    ? [...new Set(value.filter((item): item is string => typeof item === "string"))]
    : [];
}

/** Anything unrecognised is dropped: a stale record costs a name, never the list. */
export function readRepoList(raw: unknown): RepoList {
  if (typeof raw !== "object" || raw === null) return { closed: [], names: {}, pinned: [] };
  const record = raw as Record<string, unknown>;
  const names: Record<string, string> = {};
  if (typeof record.names === "object" && record.names !== null) {
    for (const [root, name] of Object.entries(record.names)) {
      if (typeof name === "string" && name.trim() !== "") names[root] = name;
    }
  }
  return { closed: strings(record.closed), names, pinned: strings(record.pinned) };
}

export function folderName(root: string): string {
  const parts = root.replace(/[\\/]+$/, "").split(/[\\/]/);
  return parts[parts.length - 1] || root;
}

export function markClosed(list: RepoList, root: string): RepoList {
  return list.closed.includes(root) ? list : { ...list, closed: [...list.closed, root] };
}

export function markOpened(list: RepoList, root: string): RepoList {
  return list.closed.includes(root)
    ? { ...list, closed: list.closed.filter((each) => each !== root) }
    : list;
}

/** Out of the list altogether: closed, name and pin go with it. */
export function forget(list: RepoList, root: string): RepoList {
  const names = { ...list.names };
  delete names[root];
  return {
    closed: list.closed.filter((each) => each !== root),
    names,
    pinned: list.pinned.filter((each) => each !== root),
  };
}

/** An empty name, or the folder's own, clears the override. */
export function rename(list: RepoList, root: string, name: string): RepoList {
  const trimmed = name.trim();
  const names = { ...list.names };
  if (trimmed === "" || trimmed === folderName(root)) delete names[root];
  else names[root] = trimmed;
  return { ...list, names };
}

export function togglePin(list: RepoList, root: string): RepoList {
  return list.pinned.includes(root)
    ? { ...list, pinned: list.pinned.filter((each) => each !== root) }
    : { ...list, pinned: [...list.pinned, root] };
}

/** Open and closed rows together: the pinned first, in pin order, then by name. Groups
    keep this order inside each bucket, so a pin puts a row at the top of its group. */
export function listedRepos(open: readonly RepoOverview[], list: RepoList): ListedRepo[] {
  const rows: ListedRepo[] = open.map((overview) => ({
    root: overview.root,
    name: list.names[overview.root] ?? overview.name,
    overview,
    pinned: list.pinned.includes(overview.root),
  }));
  const shown = new Set(rows.map((row) => row.root));
  for (const root of list.closed) {
    if (shown.has(root)) continue;
    shown.add(root);
    rows.push({
      root,
      name: list.names[root] ?? folderName(root),
      overview: null,
      pinned: list.pinned.includes(root),
    });
  }
  const pinRank = (row: ListedRepo) => (row.pinned ? list.pinned.indexOf(row.root) : Infinity);
  return rows.sort(
    (a, b) =>
      pinRank(a) - pinRank(b) ||
      a.name.localeCompare(b.name, undefined, { sensitivity: "base" }) ||
      a.root.localeCompare(b.root),
  );
}
