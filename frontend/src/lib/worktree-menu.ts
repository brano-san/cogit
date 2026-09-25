import type { ContextItem, WorktreeEntry } from "./ipc";
import { SEPARATOR, item, offer, tidy } from "./context-menu";
import { removable } from "./worktree-list";

/** Not `worktree-`: the palette's Remove Worktree… and Prune Obsolete Worktrees… are
    `worktree-remove` and `worktree-prune`, and a menu bar item must not land here. */
const PREFIX = "worktree-row-";

const COMMANDS = ["open", "reveal", "copy", "lock", "unlock", "remove", "prune", "repair"] as const;
export type WorktreeCommand = (typeof COMMANDS)[number];

const id = (command: WorktreeCommand) => `${PREFIX}${command}`;

/** Git refuses to forget a locked worktree, gone or not, until it is unlocked. */
export function pruneBlocked(entry: WorktreeEntry): string | null {
  return entry.locked === null ? null : "locked: unlock it first";
}

/** The chosen item comes back as a `menu-command`, like every popup menu (R-260). */
export function worktreeMenu(entry: WorktreeEntry): ContextItem[] {
  const lock =
    entry.locked === null
      ? item(id("lock"), "Lock", !entry.isMain)
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
    item(id("open"), "Open", !entry.isCurrent),
    item(id("reveal"), "Reveal in File Manager"),
    item(id("copy"), "Copy Path"),
    SEPARATOR,
    lock,
    item(id("remove"), "Remove…", removable(entry)),
  ]);
}

export function parseWorktreeCommand(chosen: string): WorktreeCommand | null {
  if (!chosen.startsWith(PREFIX)) return null;
  const command = chosen.slice(PREFIX.length);
  return (COMMANDS as readonly string[]).includes(command) ? (command as WorktreeCommand) : null;
}
