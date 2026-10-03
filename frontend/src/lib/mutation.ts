import type { RepoId, WorkingState } from "$lib/ipc";

export interface MutationContext {
  repo: () => RepoId | null;
  /** `repository.epoch`: a change of it means the panels show another repository now. */
  epoch: () => number;
  report: (err: unknown) => void;
  loadWorktree: (repo: RepoId) => Promise<WorkingState | null>;
  /** `state`: the counters of the list read just before, so the status is not walked again. */
  after: (paths: string[], state: WorkingState | null) => Promise<void>;
}

/** Every change to the working tree ends the same way: reload it and refresh what depends
    on it, or report why not. `false` means nothing was done. A step that `readsBack` has
    read the list itself already — Stage, Unstage, Discard through the worktree store — and
    a second `worktree_files` would only repeat it, and its answer carries the counters too
    (doc/12-risks.md, R-316). */
export async function runMutation(
  context: MutationContext,
  step: (repo: RepoId) => Promise<unknown>,
  paths: string[],
  readsBack: boolean,
  /** The repository a dialog was opened on, when that may no longer be the shown one. */
  repo?: RepoId,
): Promise<boolean> {
  const id = repo ?? context.repo();
  if (!id) return false;
  const epoch = context.epoch();
  let result: unknown;
  try {
    result = await step(id);
  } catch (err) {
    context.report(err);
    return false;
  }
  if (context.epoch() !== epoch || context.repo() !== id) return true;
  const state = readsBack ? ((result as WorkingState | null | undefined) ?? null) : await context.loadWorktree(id);
  await context.after(paths, state);
  return true;
}
