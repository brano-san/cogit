import type { ListFile } from "./file-view";
import type { FileEntry } from "./ipc/bindings";

export interface FilesPanelInput {
  /** A repository is open and the panel has something behind it. */
  content: boolean;
  stash: { worktree: readonly unknown[]; index: readonly unknown[]; untracked: readonly unknown[] } | null;
  /** The comparison's files while the graph still has its commit selected, else null. */
  compare: readonly unknown[] | null;
  onWorkingTree: boolean;
  worktree: number;
  /** Rows of the selected commit's list, the Unchanged ones included when shown. */
  commit: number;
}

/** What an empty list says: nothing while its answer is on the way, and nothing that passes
    for an answer when reading failed — the notification says why. */
export function emptyText(read: { settled: boolean; failed: boolean }, empty: string): string {
  if (read.failed) return "The files could not be listed.";
  return read.settled ? empty : "";
}

export type FilesPanelList = "none" | "stash" | "compare" | "worktree" | "commit";

/** Which list the Files panel shows and how many rows it has: the header and the body
    read this one answer, so they cannot disagree (frontend/CLAUDE.md, State). */
export function filesPanelList(input: FilesPanelInput): { kind: FilesPanelList; count: number | undefined } {
  if (!input.content) return { kind: "none", count: undefined };
  if (input.stash) {
    const { worktree, index, untracked } = input.stash;
    return { kind: "stash", count: worktree.length + index.length + untracked.length };
  }
  if (input.compare) return { kind: "compare", count: input.compare.length };
  if (input.onWorkingTree) return { kind: "worktree", count: input.worktree };
  return { kind: "commit", count: input.commit };
}

/** The working tree while its index is not shown apart (#32): one row per path. A file changed
    again on disk after staging keeps what the index says of it (added, renamed), unless it is
    gone from disk or in conflict, which is the news. */
export function mergeIndex(unstaged: readonly FileEntry[], staged: readonly FileEntry[]): ListFile[] {
  const inIndex = new Map(staged.map((file) => [file.path, file]));
  const rows: ListFile[] = [];
  for (const file of unstaged) {
    const indexed = inIndex.get(file.path);
    if (!indexed) {
      rows.push(file);
      continue;
    }
    inIndex.delete(file.path);
    const base = file.status === "modified" ? indexed : file;
    rows.push({ ...base, modeChange: file.modeChange ?? indexed.modeChange, indexState: "partly" });
  }
  for (const file of inIndex.values()) rows.push({ ...file, indexState: "staged" });
  return rows;
}

/** Which diff a row of the one list opens: what is left to stage, else what is staged. */
export function mergedSide(path: string, unstaged: readonly FileEntry[]): "worktree" | "index" {
  return unstaged.some((file) => file.path === path) ? "worktree" : "index";
}

/** The staged files the list shows once filtered, for Commit What You See: the second
    section while the index is apart, the staged rows of the one list otherwise. */
export function stagedShown(
  shown: readonly (readonly string[])[],
  separate: boolean,
  staged: readonly FileEntry[],
): string[] {
  if (separate) return [...(shown[1] ?? [])];
  const inIndex = new Set(staged.map((file) => file.path));
  return (shown[0] ?? []).filter((path) => inIndex.has(path));
}
