import type { RepoId } from "$lib/ipc";

export interface MutationContext {
  repo: () => RepoId | null;
  /** `repository.epoch`: a change of it means the panels show another repository now. */
  epoch: () => number;
  report: (err: unknown) => void;
  loadWorktree: (repo: RepoId) => Promise<void>;
  after: (paths: string[]) => Promise<void>;
}

/** Every change to the working tree ends the same way: reload it and refresh what depends
    on it, or report why not. `false` means nothing was done. A step that `readsBack` has
    read the list itself already — Stage, Unstage, Discard through the worktree store — and
    a second `worktree_files` would only repeat it (doc/12-risks.md, R-316). */
export async function runMutation(
  context: MutationContext,
  step: (repo: RepoId) => Promise<unknown>,
  paths: string[],
  readsBack: boolean,
): Promise<boolean> {
  const id = context.repo();
  if (!id) return false;
  const epoch = context.epoch();
  try {
    await step(id);
  } catch (err) {
    context.report(err);
    return false;
  }
  if (context.epoch() !== epoch) return true;
  if (!readsBack) await context.loadWorktree(id);
  await context.after(paths);
  return true;
}
