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
