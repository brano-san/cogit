import { shortOid } from "$lib/format";
import type { Branch, FileEntry, WorktreeEntry } from "$lib/ipc";

export interface WorktreeTag {
  id: "primary" | "open" | "bare" | "locked" | "missing" | "dirty";
  label: string;
  tooltip: string;
}

/** The folder as the row shows it inline: from the main worktree's parent on, so siblings
    and nested ones read short; elsewhere in full. The tooltip has the full path. */
export function shortWorktreePath(path: string, mainPath: string | undefined): string {
  const cut = mainPath?.lastIndexOf("/") ?? -1;
  if (mainPath === undefined || cut <= 0) return path;
  const parent = mainPath.slice(0, cut + 1);
  return path.toLowerCase().startsWith(parent.toLowerCase()) ? `…/${path.slice(parent.length)}` : path;
}

/** `3 changed, 2 untracked`: what the bare `dirty` mark used to leave unsaid. */
export function changeCounts(entry: WorktreeEntry): string {
  const parts: string[] = [];
  if (entry.changed > 0) parts.push(`${entry.changed} changed`);
  if (entry.untracked > 0) parts.push(`${entry.untracked} untracked`);
  return parts.length > 0 ? parts.join(", ") : "changes";
}

/** What the row says after the folder name: the branch, or where HEAD is detached. */
export function worktreeWhere(entry: WorktreeEntry): string {
  if (entry.branch) return entry.branch;
  if (entry.missing && entry.head === "") return "";
  return entry.head === ""
    ? "no commit yet"
    : `detached at ${shortOid(entry.head)}`;
}

export function worktreeTags(entry: WorktreeEntry): WorktreeTag[] {
  const tags: WorktreeTag[] = [];
  // Not "main": beside a branch called main or master it read as one more branch.
  if (entry.isMain) {
    tags.push({
      id: "primary",
      label: "primary",
      tooltip: "Main worktree: the folder that holds the repository. It cannot be removed or moved.",
    });
  }
  if (entry.isCurrent) {
    tags.push({ id: "open", label: "open", tooltip: "Open in Cogit: the panels show this worktree" });
  }
  if (entry.bare) {
    tags.push({ id: "bare", label: "bare", tooltip: "Bare repository: no files are checked out here" });
  }
  if (entry.locked !== null) {
    tags.push({
      id: "locked",
      label: "locked",
      tooltip:
        `Locked${entry.locked ? `: ${entry.locked}` : ""}. Prune and Remove leave it alone ` +
        "until it is unlocked.",
    });
  }
  if (entry.missing) {
    tags.push({
      id: "missing",
      label: "missing",
      tooltip:
        (entry.locked === null
          ? "Missing (prunable): the folder is not there. Prune forgets the registration; "
          : "Missing: the folder is not there, and Git keeps a locked one until it is unlocked. ") +
        "Repair points it at the folder's new place.",
    });
  } else if (entry.dirty) {
    tags.push({
      id: "dirty",
      label: changeCounts(entry),
      tooltip: `Uncommitted changes: ${changeCounts(entry)}. Commit or stash them before removing it.`,
    });
  }
  return tags;
}

/** The rows to show: the main worktree alone is no list, just the empty state. */
export function listedRows(entries: readonly WorktreeEntry[]): readonly WorktreeEntry[] {
  return entries.some((entry) => !entry.isMain) ? entries : [];
}

/** The header's `Worktrees (N)`: the linked ones, as `git worktree list` minus the main. */
export function linkedCount(entries: readonly WorktreeEntry[]): number {
  return entries.filter((entry) => !entry.isMain).length;
}

/** What `git worktree prune` forgets: a registration whose folder is gone, unless it is
    locked — git's own `prunable` in `worktree list --porcelain`, without the expiry, which
    a plain `prune` does not apply. */
export function prunable(entries: readonly WorktreeEntry[]): WorktreeEntry[] {
  return entries.filter((entry) => entry.missing && entry.locked === null);
}

export function hasStale(entries: readonly WorktreeEntry[]): boolean {
  return prunable(entries).length > 0;
}

export interface PruneAllButton {
  label: string;
  disabled: boolean;
  tip: string;
}

/** Off with the reason while there is nothing to prune; on, it says how many. */
export function pruneAllButton(entries: readonly WorktreeEntry[]): PruneAllButton {
  const count = prunable(entries).length;
  if (count > 0) {
    return {
      label: `Prune All (${count})…`,
      disabled: false,
      tip: `Forget ${count === 1 ? "the worktree" : `${count} worktrees`} whose folder is gone`,
    };
  }
  const lockedOnly = entries.some((entry) => entry.missing);
  return {
    label: "Prune All…",
    disabled: true,
    tip: lockedOnly
      ? "Nothing to prune: the missing worktrees are locked"
      : "Nothing to prune: no worktree folder is missing",
  };
}

/** Another worktree's folder can change unseen: the watcher follows only the repository on
    screen, so its dirty mark needs reading again now and then. */
export function othersToWatch(entries: readonly WorktreeEntry[]): boolean {
  return entries.some((entry) => !entry.isCurrent && !entry.missing);
}

/** The main copy stays, the one on screen is not pulled out from under the panels, and a
    missing one is pruned, not removed. A locked one needs `remove -f -f`, which Cogit never runs. */
export function removable(
  entry: WorktreeEntry | undefined,
): entry is WorktreeEntry {
  return (
    entry !== undefined &&
    !entry.isMain &&
    !entry.isCurrent &&
    !entry.missing &&
    entry.locked === null
  );
}

export interface RemovalNeeds {
  dirty: boolean;
  submodules: boolean;
  /** Git removes the worktree only with `--force`, which the dialog asks for separately. */
  force: boolean;
}

/** `changes` is `null` while they are still being read. Git refuses a worktree with
    submodules checked out however clean it is, so that takes `--force` too. */
export function removalNeeds(
  entry: WorktreeEntry,
  changes: readonly FileEntry[] | null,
): RemovalNeeds {
  if (changes === null)
    return { dirty: false, submodules: false, force: false };
  const dirty = changes.length > 0;
  const submodules = entry.hasSubmodules;
  return { dirty, submodules, force: dirty || submodules };
}

/** `[origin/x: gone]`: the config still names the tracking branch, but a pruning fetch
    took its ref, so ahead and behind count against nothing. */
export function upstreamGone(
  branch: Branch,
  branches: readonly Branch[],
): boolean {
  return (
    branch.upstream !== null &&
    !branches.some((other) => other.name === branch.upstream)
  );
}

export type WorktreeState = "changes" | "synced" | "unpushed" | "missing";

/** A branch another worktree has checked out, as Branches marks it (#25). */
export interface WorktreeMark {
  path: string;
  state: WorktreeState;
}

/** The one on screen is left out: its branch is HEAD, which says so already. A clean
    worktree counts as synced only when its branch tracks something and is not ahead. */
export function worktreeMarks(
  entries: readonly WorktreeEntry[],
  branches: readonly Branch[] = [],
): Map<string, WorktreeMark> {
  const locals = new Map(
    branches
      .filter((branch) => branch.kind === "local")
      .map((branch) => [branch.name, branch]),
  );
  const marks = new Map<string, WorktreeMark>();
  for (const entry of entries) {
    if (entry.isCurrent || !entry.branch) continue;
    const branch = locals.get(entry.branch);
    const pushed =
      branch !== undefined &&
      branch.upstream !== null &&
      !upstreamGone(branch, branches) &&
      branch.ahead === 0;
    const state: WorktreeState = entry.missing
      ? "missing"
      : entry.dirty
        ? "changes"
        : pushed
          ? "synced"
          : "unpushed";
    marks.set(entry.branch, { path: entry.path, state });
  }
  return marks;
}

const MARK_STATE: Record<WorktreeState, string> = {
  missing: "its folder is missing",
  changes: "it has uncommitted changes",
  synced: "it is clean and pushed",
  unpushed: "it is clean, with commits not pushed",
};

export function worktreeMarkTooltip(mark: WorktreeMark): string {
  return `Checked out in the worktree ${mark.path}; ${MARK_STATE[mark.state]}`;
}
