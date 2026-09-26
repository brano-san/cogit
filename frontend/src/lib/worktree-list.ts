import { shortOid } from "$lib/format";
import type { Branch, FileEntry, WorktreeEntry } from "$lib/ipc";
import { branchNameProblem } from "$lib/names";

export interface WorktreeTag {
  id: "main" | "locked" | "missing" | "dirty";
  label: string;
  tooltip: string;
}

/** What the row says after the folder name: the branch, or where HEAD is detached. */
export function worktreeWhere(entry: WorktreeEntry): string {
  if (entry.branch) return entry.branch;
  if (entry.missing && entry.head === "") return "";
  return entry.head === "" ? "no commit yet" : `detached at ${shortOid(entry.head)}`;
}

export function worktreeTags(entry: WorktreeEntry): WorktreeTag[] {
  const tags: WorktreeTag[] = [];
  if (entry.isMain) {
    tags.push({
      id: "main",
      label: "main",
      tooltip: "Main working copy: the folder that holds .git. It cannot be removed.",
    });
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
        "Missing: the folder is not there. Prune forgets the registration; Repair points it " +
        "at the folder's new place.",
    });
  } else if (entry.dirty) {
    tags.push({
      id: "dirty",
      label: "dirty",
      tooltip: "Dirty: uncommitted changes in this worktree. Commit or stash them before removing it.",
    });
  }
  return tags;
}

export function hasStale(entries: readonly WorktreeEntry[]): boolean {
  return entries.some((entry) => entry.missing);
}

/** Another worktree's folder can change unseen: the watcher follows only the repository on
    screen, so its dirty mark needs reading again now and then. */
export function othersToWatch(entries: readonly WorktreeEntry[]): boolean {
  return entries.some((entry) => !entry.isCurrent && !entry.missing);
}

/** The main copy stays, the one on screen is not pulled out from under the panels, and a
    missing one is pruned, not removed. */
export function removable(entry: WorktreeEntry | undefined): entry is WorktreeEntry {
  return entry !== undefined && !entry.isMain && !entry.isCurrent && !entry.missing;
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
  if (changes === null) return { dirty: false, submodules: false, force: false };
  const dirty = changes.length > 0;
  const submodules = entry.hasSubmodules;
  return { dirty, submodules, force: dirty || submodules };
}

export interface BranchChoice {
  name: string;
  /** Where the branch is checked out already; such a branch cannot go in a new worktree. */
  heldBy: string | null;
}

export function branchChoices(
  branches: readonly Branch[],
  entries: readonly WorktreeEntry[],
): BranchChoice[] {
  const held = new Map(
    entries.filter((entry) => entry.branch).map((entry) => [entry.branch as string, entry.path]),
  );
  return branches
    .filter((branch) => branch.kind === "local")
    .map((branch) => ({ name: branch.name, heldBy: held.get(branch.name) ?? null }));
}

/** Why the Add Worktree dialog cannot go ahead yet, or null when it can. */
export function addProblem(input: {
  folder: string;
  create: boolean;
  branch: string;
  choices: readonly BranchChoice[];
}): string | null {
  if (input.folder.trim() === "") return "Choose a folder for the worktree.";
  const name = input.branch.trim();
  if (name === "") return input.create ? "Enter a name for the new branch." : "Choose a branch.";
  const existing = input.choices.find((choice) => choice.name === name);
  if (input.create) {
    if (existing) return `${name} already exists; pick it under Existing branch.`;
    return branchNameProblem(name, input.choices.map((choice) => choice.name));
  }
  if (!existing) return "Choose a branch.";
  if (existing.heldBy) {
    return `${name} is checked out in ${existing.heldBy}. Git keeps a branch in one worktree at a time.`;
  }
  return null;
}

/** `[origin/x: gone]`: the config still names the tracking branch, but a pruning fetch
    took its ref, so ahead and behind count against nothing. */
export function upstreamGone(branch: Branch, branches: readonly Branch[]): boolean {
  return branch.upstream !== null && !branches.some((other) => other.name === branch.upstream);
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
    branches.filter((branch) => branch.kind === "local").map((branch) => [branch.name, branch]),
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
