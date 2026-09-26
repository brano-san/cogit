export type StashMode = "all" | "keepIndex" | "keepWorktree";

export interface StashChoice {
  mode: StashMode;
  message: string;
}

export const STASH_MODES: readonly { mode: StashMode; label: string; hint: string }[] = [
  {
    mode: "all",
    label: "Stash All",
    hint: "Stash every change, untracked files included, and clean the working tree",
  },
  {
    mode: "keepIndex",
    label: "+ Keep Index",
    hint: "Stash every change, but leave the staged ones in the index and the working tree",
  },
  {
    mode: "keepWorktree",
    label: "+ Keep Working Tree",
    hint: "Stash the changes to tracked files and leave every file as it is",
  },
];

/** The footer after Cancel, left to right: the primary Stash All rightmost (R-167). */
export const STASH_BUTTONS = [...STASH_MODES.slice(1), ...STASH_MODES.slice(0, 1)];

export function stashNameProblem(name: string): string | null {
  return name.trim() === "" ? "Enter a name." : null;
}

export type StashRequest =
  | { kind: "push"; message: string; includeUntracked: boolean; keepIndex: boolean }
  | { kind: "keepWorktree"; message: string };

/** What each mode asks Git for. The quick variants pass an empty message: Git's own. */
export function stashRequest(choice: StashChoice): StashRequest {
  const message = choice.message.trim();
  if (choice.mode === "keepWorktree") return { kind: "keepWorktree", message };
  return { kind: "push", message, includeUntracked: true, keepIndex: choice.mode === "keepIndex" };
}
