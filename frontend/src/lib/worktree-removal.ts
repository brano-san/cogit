import type { WorktreeEntry } from "$lib/ipc";
import type { ConfirmRequest } from "$stores/confirm.svelte";

/** What removing a worktree needs from the app: the store's calls, the error window, the
    Undo list and the question dialog. */
export interface RemovalHost {
  remove: (path: string, force: boolean) => Promise<void>;
  fail: (error: unknown, title: string) => void;
  refreshSafety: () => Promise<void>;
  /** Whether the list, re-read from git after the removal, still has the path. */
  listed: (path: string) => boolean;
  leftover: (path: string) => Promise<boolean>;
  deleteLeftover: (path: string) => Promise<void>;
  ask: (request: ConfirmRequest) => Promise<boolean>;
}

/** Removes the worktree; on a failure the list says what is left (R-184): git 2.51 drops the
    registration before it deletes the folder, and a lock or a running program keeps the
    folder. Still registered → offer --force once; gone from the list → offer to delete what
    is left on disk. */
export async function removeWorktree(target: WorktreeEntry, force: boolean, host: RemovalHost): Promise<void> {
  let failed = false;
  await host.remove(target.path, force).catch((err: unknown) => {
    failed = true;
    host.fail(err, "Could not remove the worktree");
  });
  await host.refreshSafety();
  if (!failed) return;
  if (host.listed(target.path)) {
    if (force) return;
    const yes = await host.ask({
      title: "Force Remove Worktree",
      message: `Git could not remove ${target.name}, and it is still registered. Retry with --force? Uncommitted work is put in a stash first.`,
      confirm: "Force Remove",
      warning: true,
    });
    if (yes) await removeWorktree(target, true, host);
    return;
  }
  if (!(await host.leftover(target.path))) return;
  const yes = await host.ask({
    title: "Delete Leftover Folder",
    message:
      `Git no longer lists ${target.name} but could not delete its folder, so part of it is still on disk ` +
      "(something may be using it, such as a terminal or an editor; close it first). Delete what is left?",
    confirm: "Delete Folder",
    warning: true,
    items: [target.path],
  });
  if (!yes) return;
  await host
    .deleteLeftover(target.path)
    .catch((err: unknown) => host.fail(err, "Could not delete the leftover folder"));
}
