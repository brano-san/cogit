import { commands } from "./bindings";
import type { DesktopInfo, GitError, IndexEditorSides, IndexFlag, RepoId } from "./bindings";
import { CogitError } from "./index";

/** The commands behind the repository and file context menus (#36, #40, #41), kept apart
    from `index.ts` so the menus can grow without touching the shared wrapper file. */

export type { DesktopInfo, IndexEditorSides, IndexFlag };

function unwrap<T>(result: { status: "ok"; data: T } | { status: "error"; error: GitError }): T {
  if (result.status === "error") throw new CogitError(result.error);
  return result.data;
}

export async function desktopInfo() {
  return unwrap(await commands.desktopInfo());
}

export async function openOnDesktop(path: string) {
  return unwrap(await commands.openPath(path));
}

export async function revealOnDesktop(path: string) {
  return unwrap(await commands.revealPath(path));
}

export async function openPowerShell(path: string) {
  return unwrap(await commands.openPowerShell(path));
}

export async function openGitShell(path: string) {
  return unwrap(await commands.openGitShell(path));
}

export async function moveToTrash(repo: RepoId, paths: string[]) {
  return unwrap(await commands.moveToTrash(repo, paths));
}

export async function removeFromRepository(repo: RepoId, paths: string[], deleteLocal: boolean) {
  return unwrap(await commands.removeFromRepository(repo, paths, deleteLocal));
}

export async function movePath(repo: RepoId, from: string, to: string) {
  return unwrap(await commands.movePath(repo, from, to));
}

export async function setIndexFlag(repo: RepoId, paths: string[], flag: IndexFlag, on: boolean) {
  return unwrap(await commands.setIndexFlag(repo, paths, flag, on));
}

export async function indexEditorSides(repo: RepoId, path: string) {
  return unwrap(await commands.indexEditorSides(repo, path));
}

/** `null` leaves that side as it is. */
export async function writeIndexEditor(
  repo: RepoId,
  path: string,
  index: string | null,
  worktree: string | null,
) {
  return unwrap(await commands.writeIndexEditor(repo, path, index, worktree));
}

export async function saveBlob(repo: RepoId, rev: string, path: string, target: string) {
  return unwrap(await commands.saveBlob(repo, rev, path, target));
}

/** Returns where the read-only copy was written. */
export async function openReadOnly(repo: RepoId, rev: string, path: string) {
  return unwrap(await commands.openReadOnly(repo, rev, path));
}

export async function applyCommitFile(
  repo: RepoId,
  rev: string,
  path: string,
  oldPath: string | null,
  reverse: boolean,
) {
  return unwrap(await commands.applyCommitFile(repo, rev, path, oldPath, reverse));
}

export async function presentOnDisk(repo: RepoId, paths: string[]) {
  return unwrap(await commands.presentOnDisk(repo, paths));
}
