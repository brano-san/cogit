import type { FileEntry } from "./ipc/bindings";
import { DEFAULT_VIEW, type FileView } from "./file-view";

/** What the Files list is showing: the switches that mean something differ between them. */
export type ListContext = "worktree" | "commit" | "stash" | "compare";

/** The six file-state buttons, by position; the icon belongs to the slot, not the key. */
export type StateSlot = "unchanged" | "untracked" | "ignored" | "modified" | "skipped" | "missing";

export interface SwitchState {
  slot: StateSlot;
  key: keyof FileView;
  title: string;
  /** Why the switch does nothing here; `null` while it works. */
  reason: string | null;
}

export type Tool = "separateIndex" | "directories" | "regex" | "contents";

const TITLES: Record<StateSlot, string> = {
  unchanged: "If selected, unchanged files will be shown",
  untracked: "If selected, untracked files will be shown",
  ignored: "If selected, ignored files will be shown",
  modified: "If selected, modified files will be shown",
  skipped: "If selected, skip-worktree and assume-unchanged files will be shown",
  missing: "If selected, missing/removed files will be shown",
};

export const RENAME_SOURCES_TITLE =
  "If selected, removed/missing source files of detected renames will be shown";

const SLOTS: readonly StateSlot[] = [
  "unchanged",
  "untracked",
  "ignored",
  "modified",
  "skipped",
  "missing",
];

const NOUN = { commit: "A commit", stash: "A stash", compare: "A comparison" } as const;

const NO_UNCHANGED = {
  commit: null,
  stash: "Unchanged files are listed for commits, not for stashes",
  compare: "Unchanged files are listed for one commit, not for a comparison",
} as const;

function deadReason(context: Exclude<ListContext, "worktree">, slot: StateSlot): string | null {
  const noun = NOUN[context];
  switch (slot) {
    case "unchanged":
      return NO_UNCHANGED[context];
    case "untracked":
      return context === "stash" ? "Untracked files of a stash are always listed" : `${noun} has no untracked files`;
    case "ignored":
      return `${noun} has no ignored files`;
    case "modified":
      return `Every file ${noun.toLowerCase()} changed is always listed`;
    case "skipped":
      return "Skip-worktree and assume-unchanged flags belong to the working tree";
    case "missing":
      return null;
  }
}

/** On a commit, Missing/Removed keeps its place but shows the sources of renames (#3). */
export function stateSwitches(context: ListContext): SwitchState[] {
  return SLOTS.map((slot) => {
    if (context === "worktree") return { slot, key: slot, title: TITLES[slot], reason: null };
    if (slot === "missing") {
      return { slot, key: "renameSources", title: RENAME_SOURCES_TITLE, reason: null };
    }
    return { slot, key: slot, title: TITLES[slot], reason: deadReason(context, slot) };
  });
}

export function toolReason(context: ListContext, tool: Tool): string | null {
  if (context === "worktree") return null;
  if (tool === "separateIndex") {
    return context === "stash"
      ? "A stash is already listed by its parts"
      : `${NOUN[context]} has no index to separate from the working tree`;
  }
  if (tool === "contents") return "Content search reads the files on disk: select Working Tree";
  return null;
}

/** Everything a commit cannot filter by is fixed where it hides nothing. */
export const COMMIT_VIEW: FileView = {
  ...DEFAULT_VIEW,
  unchanged: false,
  untracked: true,
  ignored: false,
  skipped: false,
  renameSources: false,
  separateIndex: false,
  modified: true,
  missing: true,
  regex: false,
  contents: false,
};

const COMMIT_KEYS = ["unchanged", "renameSources", "directories", "regex"] as const;

export function commitView(stored: unknown): FileView {
  const view = { ...COMMIT_VIEW };
  if (typeof stored !== "object" || stored === null) return view;
  const source = stored as Record<string, unknown>;
  for (const key of COMMIT_KEYS) {
    const value = source[key];
    if (typeof value === "boolean") view[key] = value;
  }
  return view;
}

/** The commit's own changes first, then the rest of its tree as unchanged files. */
export function withUnchanged(
  files: readonly FileEntry[],
  tree: readonly string[] | null,
): readonly FileEntry[] {
  if (tree === null) return files;
  const changed = new Set(files.map((file) => file.path));
  const rest: FileEntry[] = [];
  for (const path of tree) {
    if (changed.has(path)) continue;
    rest.push({ path, oldPath: null, status: "unchanged", mode: "plain", modeChange: null, similarity: null });
  }
  return [...files, ...rest];
}

/** One button for directories or a flat list (#30): it shows and names what a click
    switches to, the way a list/tree button does in VS Code (R-594). */
export function layoutToggle(directories: boolean): { icon: "tree" | "flat"; title: string; next: boolean } {
  return directories
    ? { icon: "flat", title: "Show Flat List", next: false }
    : { icon: "tree", title: "Show Directories", next: true };
}
