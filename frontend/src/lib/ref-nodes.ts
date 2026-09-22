import type { Branch, CommitRow, Head, StashEntry, Tag } from "$lib/ipc";
import { shortOid } from "$lib/format";

export type RefKind =
  | "head"
  | "group"
  | "folder"
  | "local"
  | "remote-group"
  | "remote"
  | "tag"
  | "stash"
  | "lost";

export interface RefNode {
  id: string;
  kind: RefKind;
  /** Set on a node with rows under it, which is what `flatten` folds. */
  children?: boolean;
  label: string;
  depth: number;
  /** `▶` for the checked-out branch, `▷` for a remote one. */
  marker?: string;
  detail?: string;
  /** What the backend should walk from; absent on groups and folders. */
  rev?: string;
  oid?: string;
  branch?: Branch;
  /** Why this row cannot be ticked; it then counts for nothing in its group (R-158). */
  disabled?: string;
}

export interface RefTreeInput {
  head: Head | null | undefined;
  branches: readonly Branch[];
  tags: readonly Tag[];
  stashes: readonly StashEntry[];
  lost: readonly CommitRow[];
  remoteUrls: Readonly<Record<string, string>>;
  collapsed: ReadonlySet<string>;
  filter: string;
}

export type CheckState = "on" | "off" | "mixed";

function matches(node: { label: string; oid?: string }, filter: string): boolean {
  const needle = filter.trim().toLowerCase();
  if (needle === "") return true;
  return (
    node.label.toLowerCase().includes(needle) ||
    (node.oid ?? "").toLowerCase().startsWith(needle)
  );
}

function upstreamDetail(branch: Branch): string | undefined {
  if (branch.upstream === null) return undefined;
  if (branch.ahead === 0 && branch.behind === 0) {
    const remote = branch.upstream.split("/")[0] ?? branch.upstream;
    return `= ${remote}`;
  }
  return `↑${branch.ahead} ↓${branch.behind}`;
}

/** `feature/auth/login` becomes two folders and a leaf, so long prefixes are written once. */
/** `owner` is the group the folder hangs under. Without it `fix/…` under Local Branches
    and `fix/…` under origin both produced `folder:fix`; a keyed `{#each}` with a repeated
    key renders unpredictably, which is why headings expanded every other click and rows
    went missing (doc/12-risks.md, R-128). */
function withFolders(
  rows: RefNode[],
  owner: string,
  name: string,
  depth: number,
  seen: Set<string>,
  leaf: RefNode,
): void {
  const parts = name.split("/").filter((part) => part !== "");
  parts.slice(0, -1).forEach((part, index) => {
    const path = parts.slice(0, index + 1).join("/");
    if (seen.has(path)) return;
    seen.add(path);
    rows.push({
      id: `folder:${owner}/${path}`,
      kind: "folder",
      label: part,
      depth: depth + index,
      children: true,
    });
  });
  rows.push({ ...leaf, label: parts.at(-1) ?? name, depth: depth + parts.length - 1 });
}

function group(rows: RefNode[], id: string, label: string, detail?: string): void {
  rows.push({ id, kind: "group", label, depth: 0, detail, children: true });
}

/** Every row, folded or not: a heading's box is read from its children, and a child that
    is not built cannot be counted (R-158). Hiding is `flatten`'s job alone. */
export function buildRefTree(input: RefTreeInput): RefNode[] {
  const rows: RefNode[] = [];

  const headOid = input.head?.kind === "unborn" ? undefined : input.head?.oid;
  rows.push({
    id: "HEAD",
    kind: "head",
    label: headOid ? `HEAD (${shortOid(headOid)})` : "HEAD",
    depth: 0,
    rev: "HEAD",
    oid: headOid,
  });

  const locals = input.branches
    .filter((branch) => branch.kind === "local")
    .map<RefNode>((branch) => ({
      id: `local:${branch.name}`,
      kind: "local",
      label: branch.name,
      depth: 1,
      marker: branch.isHead ? "▶" : undefined,
      detail: upstreamDetail(branch),
      rev: branch.fullName,
      oid: branch.oid,
      branch,
    }))
    .filter((node) => matches(node, input.filter));

  if (locals.length > 0) {
    group(rows, "group:local", `Local Branches (${locals.length})`);
    const seen = new Set<string>();
    for (const node of locals) withFolders(rows, "local", node.label, 1, seen, node);
  }

  const remotes = new Map<string, RefNode[]>();
  for (const branch of input.branches) {
    if (branch.kind !== "remote") continue;
    const remote = branch.name.split("/")[0] ?? "origin";
    const node: RefNode = {
      id: `remote:${branch.name}`,
      kind: "remote",
      label: branch.name.slice(remote.length + 1) || branch.name,
      depth: 1,
      marker: "▷",
      rev: branch.fullName,
      oid: branch.oid,
      branch,
    };
    if (!matches({ label: branch.name, oid: branch.oid }, input.filter)) continue;
    const bucket = remotes.get(remote);
    if (bucket) bucket.push(node);
    else remotes.set(remote, [node]);
  }

  for (const [remote, branches] of [...remotes].sort(([a], [b]) => a.localeCompare(b))) {
    const id = `remote-group:${remote}`;
    group(rows, id, `${remote} (${branches.length})`, input.remoteUrls[remote]);
    const seen = new Set<string>();
    for (const node of branches) withFolders(rows, remote, node.label, 1, seen, node);
  }

  const tags = input.tags
    .map<RefNode>((tag) => ({
      id: `tag:${tag.name}`,
      kind: "tag",
      label: tag.name,
      depth: 1,
      rev: tag.fullName,
      oid: tag.oid,
      disabled: tag.pointsToCommit ? undefined : "Tag does not point to a commit",
    }))
    .filter((node) => matches(node, input.filter));
  if (tags.length > 0) {
    group(rows, "group:tags", `Tags (${tags.length})`);
    rows.push(...tags);
  }

  const stashes = input.stashes
    .map<RefNode>((entry) => ({
      id: `stash:${entry.index}`,
      kind: "stash",
      label: `stash@{${entry.index}}`,
      depth: 1,
      detail: entry.message,
      rev: `stash@{${entry.index}}`,
      oid: entry.oid,
    }))
    .filter((node) => matches(node, input.filter));
  if (stashes.length > 0) {
    group(rows, "group:stashes", `Stashes (${stashes.length})`);
    rows.push(...stashes);
  }

  const lost = input.lost
    .map<RefNode>((row) => ({
      id: `lost:${row.oid}`,
      kind: "lost",
      label: shortOid(row.oid),
      depth: 1,
      detail: row.summary,
      rev: row.oid,
      oid: row.oid,
    }))
    .filter((node) => matches(node, input.filter));
  if (lost.length > 0) {
    group(rows, "group:lost", `Lost Commits (${lost.length})`);
    rows.push(...lost);
  }

  return rows;
}

/** Every tickable row a heading owns; a leaf owns only itself. */
/** The tickable rows of a subtree: everything deeper than the node, up to the next row
    that is not. `nodes` is the whole tree, never the rows on screen. */
export function leavesUnder(nodes: readonly RefNode[], id: string): string[] {
  const at = nodes.findIndex((node) => node.id === id);
  if (at < 0) return [];
  const node = nodes[at]!;
  if (node.kind !== "group" && node.kind !== "folder") return node.disabled ? [] : [id];

  const leaves: string[] = [];
  for (const next of nodes.slice(at + 1)) {
    if (next.depth <= node.depth) break;
    if (next.rev !== undefined && !next.disabled) leaves.push(next.id);
  }
  return leaves;
}

export function checkState(
  nodes: readonly RefNode[],
  id: string,
  visible: ReadonlySet<string>,
): CheckState {
  const leaves = leavesUnder(nodes, id);
  const ticked = leaves.filter((leaf) => visible.has(leaf)).length;
  if (ticked === 0) return "off";
  return ticked === leaves.length ? "on" : "mixed";
}

/** Empty or half-ticked fills, full empties: the usual three-state box. One new set for
    the whole group, so a hundred tags are one graph reload, not a hundred. */
export function toggleNode(
  nodes: readonly RefNode[],
  id: string,
  visible: ReadonlySet<string>,
): Set<string> {
  const leaves = leavesUnder(nodes, id);
  const next = new Set(visible);
  if (checkState(nodes, id, visible) === "on") {
    for (const leaf of leaves) next.delete(leaf);
  } else {
    for (const leaf of leaves) next.add(leaf);
  }
  return next;
}

export function visibleTips(nodes: readonly RefNode[], visible: ReadonlySet<string>): string[] {
  return nodes
    .filter((node) => node.rev !== undefined && !node.disabled && visible.has(node.id))
    .map((node) => node.rev as string);
}

/** HEAD alone: a box says whether a ref's history is drawn, not whether its label is —
    labels show on every drawn commit regardless (doc/12-risks.md, R-158). */
export function defaultVisible(nodes: readonly RefNode[]): Set<string> {
  return new Set(nodes.filter((node) => node.kind === "head").map((node) => node.id));
}
