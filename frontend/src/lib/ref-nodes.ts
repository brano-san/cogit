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
  label: string;
  depth: number;
  /** `▶` for the checked-out branch, `▷` for a remote one. */
  marker?: string;
  detail?: string;
  /** What the backend should walk from; absent on groups and folders. */
  rev?: string;
  oid?: string;
  branch?: Branch;
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
function withFolders(
  rows: RefNode[],
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
    rows.push({ id: `folder:${path}`, kind: "folder", label: part, depth: depth + index });
  });
  rows.push({ ...leaf, label: parts.at(-1) ?? name, depth: depth + parts.length - 1 });
}

function group(rows: RefNode[], id: string, label: string, detail?: string): void {
  rows.push({ id, kind: "group", label, depth: 0, detail });
}

export function buildRefTree(input: RefTreeInput): RefNode[] {
  const rows: RefNode[] = [];
  const open = (id: string) => !input.collapsed.has(id);

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
    if (open("group:local")) {
      const seen = new Set<string>();
      for (const node of locals) withFolders(rows, node.label, 1, seen, node);
    }
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
    if (!open(id)) continue;
    const seen = new Set<string>();
    for (const node of branches) withFolders(rows, node.label, 1, seen, node);
  }

  const tags = input.tags
    .map<RefNode>((tag) => ({
      id: `tag:${tag.name}`,
      kind: "tag",
      label: tag.name,
      depth: 1,
      rev: tag.fullName,
      oid: tag.oid,
    }))
    .filter((node) => matches(node, input.filter));
  if (tags.length > 0) {
    group(rows, "group:tags", `Tags (${tags.length})`);
    if (open("group:tags")) rows.push(...tags);
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
    if (open("group:stashes")) rows.push(...stashes);
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
    if (open("group:lost")) rows.push(...lost);
  }

  return rows;
}

/** Every tickable row a heading owns; a leaf owns only itself. */
export function leavesUnder(nodes: readonly RefNode[], id: string): string[] {
  const at = nodes.findIndex((node) => node.id === id);
  if (at < 0) return [];
  const node = nodes[at];
  if (!node || (node.kind !== "group" && node.kind !== "folder")) return [id];

  const leaves: string[] = [];
  for (const next of nodes.slice(at + 1)) {
    if (next.kind === "group") break;
    if (next.depth <= node.depth && next.kind !== "folder") break;
    if (next.rev !== undefined) leaves.push(next.id);
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

/** A half-ticked heading clears first; the next click fills it. */
export function toggleNode(
  nodes: readonly RefNode[],
  id: string,
  visible: ReadonlySet<string>,
): Set<string> {
  const leaves = leavesUnder(nodes, id);
  const next = new Set(visible);
  if (checkState(nodes, id, visible) === "off") {
    for (const leaf of leaves) next.add(leaf);
  } else {
    for (const leaf of leaves) next.delete(leaf);
  }
  return next;
}

export function visibleTips(nodes: readonly RefNode[], visible: ReadonlySet<string>): string[] {
  return nodes
    .filter((node) => node.rev !== undefined && visible.has(node.id))
    .map((node) => node.rev as string);
}

/** What the panel starts with: the current work, not every ref in the repository. */
export function defaultVisible(nodes: readonly RefNode[]): Set<string> {
  return new Set(
    nodes.filter((node) => node.kind === "head" || node.kind === "local").map((node) => node.id),
  );
}
