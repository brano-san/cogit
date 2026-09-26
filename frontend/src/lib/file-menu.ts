import type { ContextItem } from "./ipc";
import type { FileMode } from "./ipc/bindings";
import { SEPARATOR, item, offer, submenu, tidy } from "./context-menu";

/** The Working Tree's two lists: unstaged changes are the working tree, staged the index. */
export type FileSection = "worktree" | "index";

export interface WorktreeFileTarget {
  section: FileSection;
  /** One status per file the menu acts on; the right-clicked one first. */
  statuses: readonly string[];
  staged: boolean;
  /** Some of those files have unstaged changes, untracked ones included. */
  unstaged: boolean;
  fileManager: string;
}

const NO_HISTORY = new Set(["untracked", "added"]);

/** Why these files cannot be ignored, discarded or deleted, or null when they can. The
    Files menu and the buttons on an Unstaged row read the same rules. */
function ignoreBlocked(statuses: readonly string[]): string | null {
  return statuses.length > 0 && statuses.every((status) => status === "untracked")
    ? null
    : "Only an untracked file can be ignored";
}

function discardBlocked(statuses: readonly string[]): string | null {
  return statuses.some((status) => status === "untracked")
    ? "An untracked file has no version to go back to; Delete removes it"
    : null;
}

function deleteBlocked(statuses: readonly string[]): string | null {
  return statuses.some((status) => status !== "deleted") ? null : "Not on disk";
}

/** The buttons on an Unstaged row (FilesPanel). */
export type RowAction = "stage" | "mode" | "discard" | "ignore" | "delete";

/** Why a row's button does not apply to its file, or null when it does: the menu's rule
    for the one file, so a button never does what the menu refuses. */
export function rowActionBlocked(
  action: RowAction,
  file: { status: string; modeChange: FileMode | null },
): string | null {
  switch (action) {
    case "stage":
      return null;
    case "mode":
      return file.modeChange ? null : "The mode did not change";
    case "discard":
      return discardBlocked([file.status]);
    case "ignore":
      return ignoreBlocked([file.status]);
    case "delete":
      return deleteBlocked([file.status]);
  }
}

/** The Files panel on the Working Tree, in SmartGit's order (#40). */
export function worktreeFileMenu(at: WorktreeFileTarget): ContextItem[] {
  const [status = "modified"] = at.statuses;
  const one = at.statuses.length <= 1;
  const all = (wanted: string) => at.statuses.length > 0 && at.statuses.every((s) => s === wanted);
  const untracked = at.statuses.some((s) => s === "untracked");
  const onDisk = status !== "deleted";
  const conflicted = all("conflicted");
  const history = one && !NO_HISTORY.has(status);
  const single = one ? null : "one file only";
  const worktreeOnly = at.section === "worktree" ? null : "working tree only";
  const flaggable = worktreeOnly ?? (untracked ? "untracked" : null);

  return tidy([
    offer("file-open", "Open File", single ?? (onDisk ? null : "not on disk")),
    offer("file-reveal", `Reveal in ${at.fileManager}`, single ?? (onDisk ? null : "not on disk")),
    offer("file-changes", "Show Changes", single),
    item("file-log", "Log", history),
    item("file-blame", "Blame", history, "CmdOrCtrl+Shift+L"),
    item("file-investigate", "Investigate", history, "CmdOrCtrl+Alt+Shift+L"),
    SEPARATOR,
    item("file-commit", "Commit…", !conflicted),
    item("file-stash", "Stash Selection…", !conflicted, "CmdOrCtrl+Alt+S"),
    SEPARATOR,
    item("file-stage", "Stage", at.unstaged, "CmdOrCtrl+T"),
    item("file-unstage", "Unstage", at.staged, "CmdOrCtrl+Shift+T"),
    offer("file-index-editor", "Index Editor…", single ?? (conflicted ? "resolve first" : null)),
    offer(
      "file-move",
      "Move or Rename…",
      single ?? (!onDisk ? "not on disk" : conflicted ? "resolve first" : null),
    ),
    SEPARATOR,
    submenu(
      "file-resolve",
      "Resolve",
      [item("file-resolve-theirs", "Take Theirs"), item("file-resolve-ours", "Take Ours")],
      conflicted,
    ),
    SEPARATOR,
    item("file-ignore", "Ignore", ignoreBlocked(at.statuses) === null),
    item("file-discard", "Discard…", at.unstaged && discardBlocked(at.statuses) === null, "CmdOrCtrl+Z"),
    item("file-remove", "Remove…", !untracked),
    item("file-delete", "Delete…", deleteBlocked(at.statuses) === null),
    SEPARATOR,
    item("file-copy-name", "Copy Name"),
    item("file-copy-path", "Copy Path"),
    item("file-copy-relative", "Copy Relative Path"),
    SEPARATOR,
    offer("file-assume-unchanged", "Toggle 'Assume Unchanged'", flaggable),
    offer("file-skip-worktree", "Toggle 'Skip Worktree'", flaggable),
  ]);
}

/** A path as the list shows it: the source of a rename and an unchanged file are rows of
    the list, not files of the commit. */
export function shownRow<F extends { path: string }>(
  path: string,
  rows: readonly F[],
  files: readonly F[],
): F | undefined {
  return rows.find((row) => row.path === path) ?? files.find((file) => file.path === path);
}

export interface CommitFileTarget {
  status: string;
  count: number;
  onDisk: boolean;
  fileManager: string;
}

/** A file of a commit from the history (#41). */
export function commitFileMenu(at: CommitFileTarget): ContextItem[] {
  const single = at.count <= 1 ? null : "one file only";
  const inCommit = single ?? (at.status === "deleted" ? "deleted in this commit" : null);

  return tidy([
    offer("file-changes", "Show Changes", single),
    offer("file-compare-worktree", "Compare with Working Tree", single),
    offer("file-open-version", "Open File", inCommit),
    offer(
      "file-reveal",
      `Reveal in ${at.fileManager}`,
      single ?? (at.onDisk ? null : "not in the working tree"),
      "CmdOrCtrl+Shift+E",
    ),
    SEPARATOR,
    offer("file-save-as", "Save As…", inCommit),
    offer("file-log", "Log", single),
    offer("file-blame", "Blame", inCommit, "CmdOrCtrl+Shift+L"),
    offer("file-investigate", "Investigate", inCommit, "CmdOrCtrl+Alt+Shift+L"),
    SEPARATOR,
    offer("file-cherry-pick", "Cherry-Pick", single),
    offer("file-revert", "Revert", single),
    SEPARATOR,
    item("file-copy-path", "Copy Path"),
    item("file-copy-relative", "Copy Relative Path"),
    item("file-copy-name", "Copy Name"),
  ]);
}
