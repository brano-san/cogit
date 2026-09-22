import type { ChangeKind } from "$lib/ipc";

/** What one burst of watcher events is worth re-reading. */
export type DiskPlan = {
  /** A ref moved: the repository has to be opened again, and the graph reloaded. */
  refs: boolean;
  /** The index or the working tree moved: file lists and counters. */
  worktree: boolean;
  /** A hook file was edited; only the hooks panel cares. */
  hooks: boolean;
  /** Everything hanging off a mutation — stashes, submodules, network, conflicts. */
  cascade: boolean;
};

/**
 * One `git commit` writes the index, the working tree, HEAD and a ref, and the watcher
 * reports each separately. Running the whole cascade per event meant three
 * `list_submodules` round trips for one commit (doc/12-risks.md, R-144).
 */
export function planFor(kinds: Iterable<ChangeKind>): DiskPlan {
  const seen = new Set(kinds);
  const real = [...seen].filter((kind) => kind !== "hooks");
  return {
    refs: seen.has("head") || seen.has("refs"),
    worktree: seen.has("index") || seen.has("workingTree"),
    hooks: seen.has("hooks"),
    cascade: real.length > 0,
  };
}
