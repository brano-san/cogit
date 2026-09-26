import type { Branch, CommitRow, Head, StashEntry, Tag, WorktreeEntry } from "$lib/ipc";
import { shortDate, shortOid } from "$lib/format";
import { compareDated, compareNames, DEFAULT_REF_SORT, type RefSort } from "$lib/ref-sort";
import { upstreamGone, worktreeMarks, type WorktreeMark } from "$lib/worktree-list";
import { NO_FILTER_FOLDS, shownFolds, type FilterFolds } from "$lib/tree";

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
  /** Inside a folder, only the part after it: `login` under `feature/auth`. */
  label: string;
  depth: number;
  /** The checked-out branch. */
  current?: boolean;
  detail?: string;
  /** What the backend should walk from; absent on groups and folders. */
  rev?: string;
  oid?: string;
  branch?: Branch;
  tag?: Tag;
  /** Another worktree has this branch checked out (#25). */
  worktree?: WorktreeMark;
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
  /** `cogit.tagGroupSeparator`: `/` when absent, `""` for tags without folders (#11). */
  tagSeparator?: string;
  sort?: RefSort;
  /** Tip dates by full ref name; only read while the sort goes by date (#20). */
  dates?: ReadonlyMap<string, number>;
  worktrees?: readonly WorktreeEntry[];
}

export type CheckState = "on" | "off" | "mixed";

/** Tag folders get an owner no remote can be called: a ref name cannot hold a colon. */
const TAG_OWNER = ":tags";

function matches(node: { label: string; oid?: string }, filter: string): boolean {
  const needle = filter.trim().toLowerCase();
  if (needle === "") return true;
  return (
    node.label.toLowerCase().includes(needle) ||
    (node.oid ?? "").toLowerCase().startsWith(needle)
  );
}

/** The date first: a long message is what the row cuts short. A stash keeps no timezone,
    so it is this machine's. */
function stashDate(timestamp: number): string {
  return shortDate(timestamp, -new Date(timestamp * 1000).getTimezoneOffset());
}

function upstreamDetail(branch: Branch, branches: readonly Branch[]): string | undefined {
  if (branch.upstream === null) return undefined;
  const remote = branch.upstream.split("/")[0] ?? branch.upstream;
  if (upstreamGone(branch, branches)) return `${remote}: gone`;
  if (branch.ahead === 0 && branch.behind === 0) return `= ${remote}`;
  return `↑${branch.ahead} ↓${branch.behind}`;
}

interface Level {
  folders: Map<string, Level>;
  leaves: RefNode[];
}

interface Nesting {
  /** The group the folders hang under, so `fix/…` under Local Branches and under origin
      are two folders with two ids (doc/12-risks.md, R-128). */
  owner: string;
  separator: string;
  sort: RefSort;
  dates: ReadonlyMap<string, number> | undefined;
}

function segments(name: string, separator: string): string[] {
  if (separator === "") return [name];
  const parts = name.split(separator).filter((part) => part !== "");
  return parts.length > 0 ? parts : [name];
}

/** `feature/auth/login` becomes a folder and a leaf, so a long prefix is written once. */
function nest(rows: RefNode[], leaves: readonly RefNode[], depth: number, how: Nesting): void {
  const root: Level = { folders: new Map(), leaves: [] };
  for (const leaf of leaves) {
    const parts = segments(leaf.label, how.separator);
    let level = root;
    for (const part of parts.slice(0, -1)) {
      let next = level.folders.get(part);
      if (!next) {
        next = { folders: new Map(), leaves: [] };
        level.folders.set(part, next);
      }
      level = next;
    }
    level.leaves.push({ ...leaf, label: parts.at(-1) ?? leaf.label });
  }
  emit(rows, root, [], depth, how);
}

/** Folders first on every level, then the refs. */
function emit(rows: RefNode[], level: Level, prefix: string[], depth: number, how: Nesting) {
  const folders = [...level.folders].sort(([a], [b]) => compareNames(a, b, how.sort.names));
  for (const [name, first] of folders) {
    const path = [...prefix, name];
    const label = [name];
    let folder = first;
    // A folder holding nothing but one folder is one row: `testing/network`.
    for (let only = soleFolder(folder); only; only = soleFolder(folder)) {
      path.push(only[0]);
      label.push(only[0]);
      folder = only[1];
    }
    rows.push({
      id: `folder:${how.owner}/${path.join(how.separator || "/")}`,
      kind: "folder",
      label: label.join(how.separator || "/"),
      depth,
      children: true,
    });
    emit(rows, folder, path, depth + 1, how);
  }

  const dated = (node: RefNode) => ({
    name: node.label,
    date: node.rev === undefined ? undefined : how.dates?.get(node.rev),
  });
  const leaves = [...level.leaves].sort((a, b) => compareDated(dated(a), dated(b), how.sort));
  for (const leaf of leaves) rows.push({ ...leaf, depth });
}

function soleFolder(level: Level): [string, Level] | undefined {
  if (level.leaves.length > 0 || level.folders.size !== 1) return undefined;
  return level.folders.entries().next().value;
}

function group(rows: RefNode[], id: string, label: string, detail?: string): void {
  rows.push({ id, kind: "group", label, depth: 0, detail, children: true });
}

/** Every row, folded or not: a heading's box is read from its children, and a child that
    is not built cannot be counted (R-158). Hiding is `flatten`'s job alone. */
export function buildRefTree(input: RefTreeInput): RefNode[] {
  const rows: RefNode[] = [];
  const sort = input.sort ?? DEFAULT_REF_SORT;
  const dates = sort.dates === "off" ? undefined : input.dates;
  const nesting = (owner: string, separator = "/"): Nesting => ({ owner, separator, sort, dates });

  const headOid = input.head?.kind === "unborn" ? undefined : input.head?.oid;
  rows.push({
    id: "HEAD",
    kind: "head",
    label: headOid ? `HEAD (${shortOid(headOid)})` : "HEAD",
    depth: 0,
    rev: "HEAD",
    oid: headOid,
  });

  const held = worktreeMarks(input.worktrees ?? [], input.branches);
  const locals = input.branches
    .filter((branch) => branch.kind === "local")
    .map<RefNode>((branch) => ({
      id: `local:${branch.name}`,
      kind: "local",
      label: branch.name,
      depth: 1,
      current: branch.isHead || undefined,
      detail: upstreamDetail(branch, input.branches),
      rev: branch.fullName,
      oid: branch.oid,
      branch,
      worktree: held.get(branch.name),
    }))
    .filter((node) => matches(node, input.filter));

  if (locals.length > 0) {
    group(rows, "group:local", `Local Branches (${locals.length})`);
    nest(rows, locals, 1, nesting("local"));
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
      rev: branch.fullName,
      oid: branch.oid,
      branch,
    };
    if (!matches({ label: branch.name, oid: branch.oid }, input.filter)) continue;
    const bucket = remotes.get(remote);
    if (bucket) bucket.push(node);
    else remotes.set(remote, [node]);
  }

  const byRemote = [...remotes].sort(([a], [b]) => compareNames(a, b, sort.names));
  for (const [remote, branches] of byRemote) {
    const id = `remote-group:${remote}`;
    group(rows, id, `${remote} (${branches.length})`, input.remoteUrls[remote]);
    nest(rows, branches, 1, nesting(remote));
  }

  const tags = input.tags
    .map<RefNode>((tag) => ({
      id: `tag:${tag.name}`,
      kind: "tag",
      label: tag.name,
      depth: 1,
      rev: tag.fullName,
      oid: tag.oid,
      tag,
      detail: tag.isAnnotated ? "annotated" : undefined,
      disabled: tag.pointsToCommit ? undefined : "Tag does not point to a commit",
    }))
    .filter((node) => matches(node, input.filter));
  if (tags.length > 0) {
    group(rows, "group:tags", `Tags (${tags.length})`);
    nest(rows, tags, 1, nesting(TAG_OWNER, input.tagSeparator ?? "/"));
  }

  const stashes = input.stashes
    .map<RefNode>((entry) => ({
      id: `stash:${entry.index}`,
      kind: "stash",
      label: `stash@{${entry.index}}`,
      depth: 1,
      detail: `${stashDate(entry.timestamp)} · ${entry.message}`,
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

/** While a filter is typed every folder is open: a match folded away is a match not found.
    A folder folded meanwhile is folded in `folds`, for that text only (R-240, R-485). */
export function foldedWhileFiltering(
  collapsed: ReadonlySet<string>,
  filter: string,
  folds: FilterFolds = NO_FILTER_FOLDS,
): ReadonlySet<string> {
  if (filter.trim() === "") return collapsed;
  const kept = [...collapsed].filter((id) => !id.startsWith("folder:"));
  return new Set([...kept, ...shownFolds(collapsed, filter, folds)]);
}

/** Whether a fold goes to the filter's own folds rather than the stored ones: headings of
    groups are not opened by a filter, so theirs stays stored. */
export function foldsWhileFiltering(id: string, filter: string): boolean {
  return filter.trim() !== "" && id.startsWith("folder:");
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

export interface TickState {
  state: CheckState;
  tickable: boolean;
}

/** Every row's box, as `checkState` and `leavesUnder` give it, in one pass: asked row by
    row, each box searched the whole tree again. */
export function tickStates(
  nodes: readonly RefNode[],
  visible: ReadonlySet<string>,
): Map<string, TickState> {
  const states = new Map<string, TickState>();
  const open: { id: string; depth: number; leaves: number; ticked: number }[] = [];
  const close = (depth: number) => {
    for (let top = open.at(-1); top && top.depth >= depth; top = open.at(-1)) {
      open.pop();
      const state = top.ticked === 0 ? "off" : top.ticked === top.leaves ? "on" : "mixed";
      states.set(top.id, { state, tickable: top.leaves > 0 });
    }
  };

  for (const node of nodes) {
    close(node.depth);
    if (node.kind === "group" || node.kind === "folder") {
      open.push({ id: node.id, depth: node.depth, leaves: 0, ticked: 0 });
      continue;
    }
    const ticked = !node.disabled && visible.has(node.id);
    states.set(node.id, { state: ticked ? "on" : "off", tickable: !node.disabled });
    if (node.rev === undefined || node.disabled) continue;
    for (const heading of open) {
      heading.leaves += 1;
      if (ticked) heading.ticked += 1;
    }
  }
  close(-1);
  return states;
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
