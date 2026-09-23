import { shortOid } from "$lib/format";
import type { Branch, WorktreeEntry } from "$lib/ipc";

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

/** The main copy stays, the one on screen is not pulled out from under the panels, and a
    missing one is pruned, not removed. */
export function removable(entry: WorktreeEntry | undefined): entry is WorktreeEntry {
  return entry !== undefined && !entry.isMain && !entry.isCurrent && !entry.missing;
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
    if (/[\s~^:?*[\\]/.test(name) || name.startsWith("-")) return "Git will refuse that name.";
    return null;
  }
  if (!existing) return "Choose a branch.";
  if (existing.heldBy) {
    return `${name} is checked out in ${existing.heldBy}. Git keeps a branch in one worktree at a time.`;
  }
  return null;
}

export type WorktreeState = "clean" | "changes" | "missing";

/** A branch another worktree has checked out, as Branches marks it (#25). */
export interface WorktreeMark {
  path: string;
  state: WorktreeState;
}

/** The one on screen is left out: its branch is HEAD, which says so already. */
export function worktreeMarks(entries: readonly WorktreeEntry[]): Map<string, WorktreeMark> {
  const marks = new Map<string, WorktreeMark>();
  for (const entry of entries) {
    if (entry.isCurrent || !entry.branch) continue;
    const state: WorktreeState = entry.missing ? "missing" : entry.dirty ? "changes" : "clean";
    marks.set(entry.branch, { path: entry.path, state });
  }
  return marks;
}

export function worktreeMarkTooltip(mark: WorktreeMark): string {
  const state =
    mark.state === "missing"
      ? "its folder is missing"
      : mark.state === "changes"
        ? "it has uncommitted changes"
        : "it is clean";
  return `Checked out in the worktree ${mark.path}; ${state}`;
}
