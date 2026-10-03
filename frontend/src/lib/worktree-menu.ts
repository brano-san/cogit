import type { ContextItem, WorktreeEntry } from "./ipc";
import { SEPARATOR, item, offer, tidy } from "./context-menu";
import { moveBlocked, pruneAllButton, removeBlocked } from "./worktree-list";

/** Not `worktree-`: the palette's Remove Worktree… and Prune Obsolete Worktrees… are
    `worktree-remove` and `worktree-prune`, and a menu bar item must not land here. */
const PREFIX = "worktree-row-";

const COMMANDS = [
  "open",
  "folder",
  "terminal",
  "copy",
  "lock",
  "unlock",
  "move",
  "remove",
  "prune",
  "repair",
] as const;
export type WorktreeCommand = (typeof COMMANDS)[number];

const id = (command: WorktreeCommand) => `${PREFIX}${command}`;

/** Git refuses to forget a locked worktree, gone or not, until it is unlocked. */
export function pruneBlocked(entry: WorktreeEntry): string | null {
  return entry.locked === null ? null : "locked: unlock it first";
}

/** The chosen item comes back as a `menu-command`, like every popup menu (R-260). */
export function worktreeMenu(entry: WorktreeEntry, fileManager: string): ContextItem[] {
  const lock =
    entry.locked === null
      ? offer(id("lock"), "Lock…", entry.isMain ? "the main worktree" : null)
      : item(id("unlock"), "Unlock", true);
  if (entry.missing) {
    return tidy([
      offer(id("prune"), "Prune", pruneBlocked(entry)),
      item(id("repair"), "Repair…"),
      ...(entry.locked === null ? [] : [lock]),
      SEPARATOR,
      item(id("copy"), "Copy Path"),
    ]);
  }
  return tidy([
    offer(id("open"), "Open", entry.isCurrent ? "already open" : null, "Enter"),
    item(id("folder"), `Open in ${fileManager}`),
    item(id("terminal"), "Open in Terminal"),
    item(id("copy"), "Copy Path"),
    SEPARATOR,
    lock,
    offer(id("move"), "Move…", moveBlocked(entry)),
    offer(id("remove"), "Remove…", removeBlocked(entry), "Delete"),
  ]);
}

/** The header's buttons as one menu, for a panel too narrow to show them: the palette's own
    ids, so the choice runs exactly what the palette runs. */
export function worktreeHeaderMenu(entries: readonly WorktreeEntry[]): ContextItem[] {
  const prune = pruneAllButton(entries);
  return [
    item("worktree-add", "Add Worktree…"),
    offer("worktree-prune", prune.label, prune.disabled ? "nothing to prune" : null),
  ];
}

/** The Move… field: a new folder, slashes as git lists them. */
export function moveTarget(typed: string): string {
  return typed.trim().replace(/\\/g, "/").replace(/\/+$/, "");
}

export function moveTargetProblem(typed: string, from: string): string | null {
  const to = moveTarget(typed);
  if (to === "") return "Enter the new folder";
  if (to.toLowerCase() === from.toLowerCase()) return "That is where it is now";
  return null;
}

export function parseWorktreeCommand(chosen: string): WorktreeCommand | null {
  if (!chosen.startsWith(PREFIX)) return null;
  const command = chosen.slice(PREFIX.length);
  return (COMMANDS as readonly string[]).includes(command) ? (command as WorktreeCommand) : null;
}
