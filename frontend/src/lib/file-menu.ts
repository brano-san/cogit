import type { ContextItem } from "./ipc";
import { SEPARATOR, dropStraySeparators, entry, explained, submenu } from "./menu-entries";

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

  return dropStraySeparators([
    explained("file-open", "Open File", single ?? (onDisk ? null : "not on disk")),
    explained(
      "file-reveal",
      `Reveal in ${at.fileManager}`,
      single ?? (onDisk ? null : "not on disk"),
      "CmdOrCtrl+Shift+E",
    ),
    explained("file-changes", "Show Changes", single),
    entry("file-log", "Log", history),
    entry("file-blame", "Blame", history, "CmdOrCtrl+Shift+L"),
    entry("file-investigate", "Investigate", history, "CmdOrCtrl+Alt+Shift+L"),
    SEPARATOR,
    entry("file-commit", "Commit…", !conflicted),
    entry("file-stash", "Stash Selection…", !conflicted, "CmdOrCtrl+Alt+S"),
    SEPARATOR,
    entry("file-stage", "Stage", at.unstaged, "CmdOrCtrl+T"),
    entry("file-unstage", "Unstage", at.staged, "CmdOrCtrl+Shift+T"),
    explained("file-index-editor", "Index Editor…", single ?? (conflicted ? "resolve first" : null)),
    explained(
      "file-move",
      "Move or Rename…",
      single ?? (!onDisk ? "not on disk" : conflicted ? "resolve first" : null),
    ),
    SEPARATOR,
    submenu(
      "file-resolve",
      "Resolve",
      [entry("file-resolve-theirs", "Take Theirs"), entry("file-resolve-ours", "Take Ours")],
      conflicted,
    ),
    SEPARATOR,
    entry("file-ignore", "Ignore", all("untracked")),
    entry("file-discard", "Discard…", at.unstaged && !untracked, "CmdOrCtrl+Z"),
    entry("file-remove", "Remove…", !untracked),
    entry("file-delete", "Delete…", at.statuses.some((s) => s !== "deleted")),
    SEPARATOR,
    entry("file-copy-name", "Copy Name"),
    entry("file-copy-path", "Copy Path"),
    entry("file-copy-relative", "Copy Relative Path"),
    SEPARATOR,
    explained("file-assume-unchanged", "Toggle 'Assume Unchanged'", flaggable),
    explained("file-skip-worktree", "Toggle 'Skip Worktree'", flaggable),
  ]);
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

  return dropStraySeparators([
    explained("file-changes", "Show Changes", single),
    explained("file-compare-worktree", "Compare with Working Tree", single),
    explained("file-open-version", "Open File", inCommit),
    explained(
      "file-reveal",
      `Reveal in ${at.fileManager}`,
      single ?? (at.onDisk ? null : "not in the working tree"),
      "CmdOrCtrl+Shift+E",
    ),
    SEPARATOR,
    explained("file-save-as", "Save As…", inCommit),
    explained("file-log", "Log", single),
    explained("file-blame", "Blame", inCommit, "CmdOrCtrl+Shift+L"),
    explained("file-investigate", "Investigate", inCommit, "CmdOrCtrl+Alt+Shift+L"),
    SEPARATOR,
    explained("file-cherry-pick", "Cherry-Pick", single),
    explained("file-revert", "Revert", single),
    SEPARATOR,
    entry("file-copy-path", "Copy Path"),
    entry("file-copy-relative", "Copy Relative Path"),
    entry("file-copy-name", "Copy Name"),
  ]);
}
