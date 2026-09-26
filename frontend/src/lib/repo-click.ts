import type { RepoId } from "$lib/ipc";

/** What a click on a listed repository does (#50): nothing for the one on screen, a switch
    back from what was kept for the owner of the submodule or worktree on screen, and a
    full open for any other. */
export type RepoClick = "stay" | "return" | "open";

export interface RepoClickState {
  /** The repository the panels show: the listed one, or a submodule or worktree of it. */
  shown: RepoId | null;
  /** The root an open is under way for. */
  opening: string | null;
  /** The listed repository whose submodule is on screen, if one is. */
  moduleOwner: RepoId | null;
  /** The listed repository whose worktree is on screen, if one is. */
  worktreeOwner: string | null;
}

const same = (a: string, b: string | null) =>
  b !== null && a.replaceAll("\\", "/") === b.replaceAll("\\", "/");

export function repoClick(entry: { repo: RepoId; root: string }, state: RepoClickState): RepoClick {
  if (same(entry.root, state.opening)) return "stay";
  if (state.shown !== null && state.shown.valueOf() === entry.repo.valueOf()) return "stay";
  if (same(entry.root, state.worktreeOwner)) return "return";
  if (state.moduleOwner !== null && state.moduleOwner.valueOf() === entry.repo.valueOf()) {
    return "return";
  }
  return "open";
}

/** The panels show this repository, one of its submodules or worktrees, or are opening it:
    closing it has to free them. */
export function holdsPanels(entry: { repo: RepoId; root: string }, state: RepoClickState): boolean {
  return repoClick(entry, state) !== "open";
}

/** A closed row clicked while its own open is under way: the second click of a double-click. */
export function reopenClick(root: string, opening: string | null): "stay" | "open" {
  return same(root, opening) ? "stay" : "open";
}
