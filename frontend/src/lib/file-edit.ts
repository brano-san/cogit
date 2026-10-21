import type { DiffSpec, FileStatus } from "$lib/ipc";
import type { FileMode } from "$lib/ipc/bindings";
import { showsWholeFile } from "./file-content";

/** Where Edit, a double-click or Enter on a working tree row takes the file. */
export type FileRoute =
  | { kind: "editor" }
  | { kind: "compare"; spec: DiffSpec }
  | { kind: "solver" }
  | { kind: "module" }
  | { kind: "refused"; reason: string };

export interface RowFile {
  status: FileStatus;
  mode: FileMode;
}

export interface RowPlace {
  /** The file differs from the index on disk (it is in Unstaged). */
  unstaged: boolean;
  /** HEAD is a commit: an unborn branch has nothing to compare a staged file with. */
  headExists: boolean;
}

/** Edit: one window per state. Nothing to compare — the file alone; a change — the diff with
    the file editable on the right, against the index, or against HEAD when the index already
    holds everything the file has; a conflict — the Conflict Solver. */
export function editRoute(file: RowFile, place: RowPlace): FileRoute {
  if (file.status === "conflicted") return { kind: "solver" };
  if (file.mode === "submodule") return { kind: "refused", reason: "a submodule" };
  if (file.status === "deleted" || file.status === "sparse") return { kind: "refused", reason: "not on disk" };
  if (showsWholeFile(file.status)) return { kind: "editor" };
  if (place.unstaged || !place.headExists) return { kind: "compare", spec: { kind: "workTreeVsIndex" } };
  return { kind: "compare", spec: { kind: "commitVsWorkTree", oid: "HEAD" } };
}

/** A double-click or Enter on a row of the Unstaged (`worktree`) or Staged (`index`) list:
    the side of the row clicked, whatever the Diff panel showed last. */
export function openRoute(file: RowFile, side: "worktree" | "index"): FileRoute {
  if (file.status === "conflicted") return { kind: "solver" };
  if (file.mode === "submodule") return { kind: "module" };
  if (side === "index") return { kind: "compare", spec: { kind: "indexVsHead" } };
  if (showsWholeFile(file.status)) return { kind: "editor" };
  return { kind: "compare", spec: { kind: "workTreeVsIndex" } };
}

/** A double-click or Enter on a row of a past commit: that commit's change to the file,
    read-only (`editOffer` offers Edit on a working tree side only). */
export function commitRoute(mode: FileMode, oid: string | null): FileRoute {
  if (mode === "submodule") return { kind: "module" };
  return oid ? { kind: "compare", spec: { kind: "commitVsParent", oid } } : { kind: "refused", reason: "no commit" };
}
