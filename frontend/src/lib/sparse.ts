import type { CheckState } from "$lib/ref-nodes";
import type { SparseFolder, SparsePreview, SparseState } from "$lib/ipc/bindings";

/** Repository ▸ Sparse Checkout (F-955): the folder tree's ticks as a cone set, and the
    dialog's words. */

function under(path: string, dir: string): boolean {
  return path.startsWith(`${dir}/`);
}

function parentOf(path: string): string {
  const slash = path.lastIndexOf("/");
  return slash < 0 ? "" : path.slice(0, slash);
}

/** The ticked folders without the ones a ticked folder above already takes; sorted. */
export function coneSet(folders: readonly string[]): string[] {
  const clean = [...new Set(folders.map((folder) => folder.replace(/^\/+|\/+$/g, "")).filter(Boolean))].sort();
  return clean.filter((folder) => !clean.some((other) => under(folder, other)));
}

/** On: the folder or one above it is in the set. Mixed: something under it is. */
export function folderCheck(set: readonly string[], path: string): CheckState {
  if (set.some((folder) => folder === path || under(path, folder))) return "on";
  return set.some((folder) => under(folder, path)) ? "mixed" : "off";
}

/** Ticking takes the folder with everything under it. Unticking one inside a ticked folder
    keeps its neighbours: on every level from that folder down, the other folders stay in.
    `children(dir)` lists the folders the tree has read inside `dir`. */
export function toggleFolder(
  set: readonly string[],
  path: string,
  children: (dir: string) => readonly string[],
): string[] {
  if (folderCheck(set, path) !== "on") {
    return coneSet([...set.filter((folder) => !under(folder, path)), path]);
  }
  const above = set.find((folder) => folder === path || under(path, folder));
  const kept = set.filter((folder) => folder !== above);
  if (above === undefined || above === path) return coneSet(kept);
  const neighbours: string[] = [];
  for (let level = parentOf(path); level !== "" && (level === above || under(level, above)); level = parentOf(level)) {
    const toward = childToward(level, path);
    neighbours.push(...children(level).filter((child) => child !== toward));
  }
  return coneSet([...kept, ...neighbours]);
}

/** The folder right inside `level` on the way down to `path`. */
function childToward(level: string, path: string): string {
  const slash = path.indexOf("/", level.length + 1);
  return slash < 0 ? path : path.slice(0, slash);
}

/** One row of the folder tree, in the order it is shown. */
export interface FolderRow {
  path: string;
  name: string;
  depth: number;
  folders: boolean;
  open: boolean;
}

/** The rows of the open folders, depth first; a folder not read yet shows no children. */
export function folderRows(
  read: ReadonlyMap<string, readonly SparseFolder[]>,
  open: ReadonlySet<string>,
  dir = "",
  depth = 0,
): FolderRow[] {
  const rows: FolderRow[] = [];
  for (const folder of read.get(dir) ?? []) {
    const isOpen = open.has(folder.path);
    rows.push({
      path: folder.path,
      name: folder.path.slice(folder.path.lastIndexOf("/") + 1),
      depth,
      folders: folder.folders,
      open: isOpen,
    });
    if (isOpen) rows.push(...folderRows(read, open, folder.path, depth + 1));
  }
  return rows;
}

/** The pattern editor's text: one pattern per line, blank lines and comments dropped. */
export function parsePatterns(text: string): string[] {
  return text
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .filter((line) => line.trim() !== "" && !line.startsWith("#"));
}

export function sameSet(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((item, at) => item === b[at]);
}

/** What the dialog would do; `null` when it would change nothing. */
export type SparsePlan = { kind: "set"; cone: boolean; set: string[] } | { kind: "disable" };

export function planOf(state: SparseState, enabled: boolean, set: readonly string[]): SparsePlan | null {
  if (!enabled) return state.enabled ? { kind: "disable" } : null;
  const next = state.cone ? coneSet(set) : [...set];
  const current = state.cone ? coneSet(state.set) : state.set;
  if (state.enabled && sameSet(next, current)) return null;
  return { kind: "set", cone: state.cone, set: next };
}

export function applyLabel(state: SparseState, plan: SparsePlan | null): string {
  if (plan?.kind === "disable") return "Disable";
  return state.enabled ? "Apply" : "Enable";
}

const files = (n: number) => (n === 1 ? "1 file" : `${n} files`);

/** The preview's line: what stays, what goes and what comes back. */
export function previewLine(preview: SparsePreview): string {
  const parts = [`${files(preview.present)} of ${preview.tracked} in the working tree`];
  if (preview.removed > 0) parts.push(`${files(preview.removed)} removed`);
  if (preview.appearing > 0) parts.push(`${files(preview.appearing)} brought back`);
  if (preview.removed === 0 && preview.appearing === 0) parts.push("no file comes or goes");
  return parts.join(" · ");
}

/** The repository row's tooltip for its "sparse" mark: folders in cone mode, else patterns. */
export function sparseTooltip(size: number): string {
  if (size === 0) return "Sparse checkout is on: only the top-level files";
  return `Sparse checkout is on: ${size === 1 ? "1 entry" : `${size} entries`} in its set`;
}
