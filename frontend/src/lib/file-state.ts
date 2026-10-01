import type { ConflictKind, FileMode, FileStatus, SubmoduleChange } from "$lib/ipc/bindings";
import { submoduleTooltip } from "$lib/files";

/** Which comparison a row comes from: Git calls one status letter two things by it, `M` in
    the index is staged work and `M` in the working tree is not, so the side picks the word. */
export type StateSide = "worktree" | "index" | "commit";

/** The glyph states of `FileStateIcon`. */
export type IconState =
  | "unchanged"
  | "untracked"
  | "added"
  | "modified"
  | "staged"
  | "removed"
  | "missing"
  | "renamed"
  | "conflicted"
  | "ignored";

export interface StateInput {
  status: FileStatus;
  mode?: FileMode;
  modeChange?: FileMode | null;
  submodule?: SubmoduleChange | null;
  conflict?: ConflictKind | null;
}

export interface FileState {
  /** The State column's text, whole; the cell cuts it with `…` and the tooltip has it all. */
  text: string;
  tone: "danger" | "secondary";
  icon: IconState;
  tooltip: string;
}

const CONFLICTS: Record<ConflictKind, string> = {
  bothModified: "both modified",
  bothAdded: "both added",
  bothDeleted: "both deleted",
  deletedByThem: "deleted by them",
  deletedByUs: "deleted by us",
  addedByUs: "added by us",
  addedByThem: "added by them",
};

function submoduleText(change: SubmoduleChange): string {
  const parts = [change.newCommits && "new commits", (change.modified || change.untracked) && "dirty"].filter(Boolean);
  return parts.length > 0 ? `Modified (${parts.join(", ")})` : "Modified";
}

/** The engine reports one status per list, so the side is the list the row sits in. In the
    one working-tree list (#32) a row only in the index reads as the index, and a partly staged
    one as what is left to stage: the work not yet staged is the news. */
export function sideOfRow(indexState: "staged" | "partly" | undefined, pane: StateSide): StateSide {
  if (indexState === "staged") return "index";
  if (indexState === "partly") return "worktree";
  return pane;
}

function plain(text: string, icon: IconState): Omit<FileState, "tooltip"> {
  return { text, tone: "secondary", icon };
}

function describe(file: StateInput, side: StateSide): Omit<FileState, "tooltip"> {
  switch (file.status) {
    case "conflicted":
      return {
        text: file.conflict ? `Conflicted (${CONFLICTS[file.conflict]})` : "Conflicted",
        tone: "danger",
        icon: "conflicted",
      };
    case "untracked":
      return plain("Untracked", "untracked");
    case "ignored":
      return plain("Ignored", "ignored");
    case "added":
      return plain("Added", "added");
    case "renamed":
      return plain("Renamed", "renamed");
    case "copied":
      return plain("Copied", "renamed");
    case "deleted":
      return side === "worktree" ? plain("Missing", "missing") : plain("Removed", "removed");
    case "modified":
      // ponytail: the engine reports only the new mode, so a change to a regular file is not seen
      if (file.modeChange === "symlink" || file.modeChange === "submodule") return plain("Type changed", "modified");
      if (file.submodule && side === "worktree") return plain(submoduleText(file.submodule), "modified");
      return side === "index" ? plain("Staged", "staged") : plain("Modified", "modified");
    case "assumeUnchanged":
      return plain("Assume unchanged", "unchanged");
    case "skipped":
      return plain("Skipped", "unchanged");
    case "sparse":
      return plain("Outside sparse checkout", "unchanged");
    case "unchanged":
      return plain("Unchanged", "unchanged");
  }
}

export function fileState(file: StateInput, side: StateSide): FileState {
  const state = describe(file, side);
  const tooltip = file.submodule && side === "worktree" ? `${state.text}\n${submoduleTooltip(file.submodule)}` : state.text;
  return { ...state, tooltip };
}
