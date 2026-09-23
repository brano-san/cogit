import type { ContextItem } from "./ipc";

export const SEPARATOR: ContextItem = { id: "", label: "", enabled: false, separator: true };

export function item(id: string, label: string, enabled = true, accelerator?: string): ContextItem {
  return { id, label, enabled, separator: false, accelerator: accelerator ?? null };
}

/** A row that is off says why in its own label: a native menu has no tooltip for a
    disabled item (doc/12-risks.md, R-250). `null` means the row is on. */
export function offer(
  id: string,
  label: string,
  blocked: string | null,
  accelerator?: string,
): ContextItem {
  return blocked === null
    ? item(id, label, true, accelerator)
    : item(id, `${label} (${blocked})`, false, accelerator);
}

/** Drops separators that separate nothing — leading, trailing and doubled ones — so a menu
    can be written as a flat list and each row can switch itself off (the same rule
    `menu::tidy` applies in Rust, R-133). Every context menu goes through it. */
export function tidy(entries: readonly ContextItem[]): ContextItem[] {
  const kept: ContextItem[] = [];
  for (const entry of entries) {
    if (entry.separator && (kept.length === 0 || kept[kept.length - 1]!.separator)) continue;
    kept.push(entry);
  }
  while (kept.length > 0 && kept[kept.length - 1]!.separator) kept.pop();
  return kept;
}

/** A row of the Repositories panel: opening it, and the three things the OS can do. */
export function repoMenu(at: { active: boolean }): ContextItem[] {
  return [
    item("repo-open", "Open this repository", !at.active),
    SEPARATOR,
    item("repo-explorer", "Show in Explorer"),
    item("repo-terminal", "Open in Terminal"),
    item("repo-copy-path", "Copy the path"),
    SEPARATOR,
    item("repo-close", "Close this repository"),
  ];
}

export function refMenu(at: { kind: string }): ContextItem[] {
  if (at.kind !== "lost") return [];
  return [item("restore-lost", "Create a branch here"), SEPARATOR, item("copy-sha", "Copy the full SHA")];
}

/** A row of the Files panel, in SmartGit's order: what you do to a file most often comes
    first, and what cannot be undone sits behind a separator. Chords match
    doc/11-keybindings.md — a menu that shows a different one teaches the wrong thing. */
export function fileMenu(at: {
  status: string;
  staged: boolean;
  count: number;
  /** A file of a past commit has nothing to stage and nothing on disk to discard. */
  worktree: boolean;
}): ContextItem[] {
  const untracked = at.status === "untracked";
  const one = at.count <= 1;
  const hasHistory = !untracked && at.status !== "added" && at.status !== "deleted";

  const staging: ContextItem[] = at.worktree
    ? [
        item("file-stage", "Stage", !at.staged, "CmdOrCtrl+T"),
        item("file-unstage", "Unstage", at.staged, "CmdOrCtrl+Shift+T"),
        SEPARATOR,
        item("file-discard", "Discard changes…", !untracked, "CmdOrCtrl+Z"),
        item("file-ignore", "Add to .gitignore", untracked),
        item("file-delete", "Delete from disk…", untracked),
        SEPARATOR,
      ]
    : [];

  return tidy([
    ...staging,
    item("file-blame", "Blame", hasHistory && one, "CmdOrCtrl+Shift+L"),
    item("file-history", "History of this file", hasHistory && one),
    SEPARATOR,
    item("file-explorer", "Show in Explorer", one && at.worktree, "CmdOrCtrl+Shift+E"),
    item("file-copy-path", "Copy the path", one),
  ]);
}
