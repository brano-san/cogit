import type { RepoId } from "$lib/ipc";

export interface WorkingTreeFiles {
  staged: readonly { path: string }[];
  unstaged: readonly { path: string; status: string }[];
}

/** What the Working Tree row's menu (`RefActions`) borrows from App and the stores. */
export interface WorkingTreeHost {
  stage(repo: RepoId, paths: string[]): Promise<void>;
  unstage(repo: RepoId, paths: string[]): Promise<void>;
  discard(repo: RepoId, paths: string[]): Promise<void>;
  /** App's `mutate`: runs the write and refreshes what depends on `paths`, the open diff
      among them. */
  mutate(step: (repo: RepoId) => Promise<unknown>, paths: string[], readsBack: boolean): Promise<boolean>;
  confirmDiscard(paths: string[]): Promise<boolean>;
  /** Shows the Commit Message panel if it is hidden, then puts the cursor in it. */
  focusCommit(): Promise<unknown>;
}

/** Stage, Unstage, Discard and Commit of the Working Tree row in the graph. */
export async function runWorkingTreeAction(name: string, files: WorkingTreeFiles, host: WorkingTreeHost): Promise<void> {
  if (name === "wt-commit") {
    await host.focusCommit();
    return;
  }
  if (name === "wt-stage") {
    const paths = files.unstaged.map((file) => file.path);
    await host.mutate((repo) => host.stage(repo, paths), paths, true);
  } else if (name === "wt-unstage") {
    const paths = files.staged.map((file) => file.path);
    await host.mutate((repo) => host.unstage(repo, paths), paths, true);
  } else if (name === "wt-discard") {
    const paths = files.unstaged.filter((file) => file.status !== "untracked").map((file) => file.path);
    if (paths.length === 0 || !(await host.confirmDiscard(paths))) return;
    await host.mutate((repo) => host.discard(repo, paths), paths, true);
  }
}
