import { commands } from "./bindings";
import type { GitError, RepoId, SparseChange, SparseFolder, SparsePreview, SparseState } from "./bindings";
import { CogitError } from "./index";

export type { SparseChange, SparseFolder, SparsePreview, SparseState };

function unwrap<T>(result: { status: "ok"; data: T } | { status: "error"; error: GitError }): T {
  if (result.status === "error") throw new CogitError(result.error);
  return result.data;
}

/** Repository ▸ Sparse Checkout (F-955). */
export async function sparseState(repo: RepoId) {
  return unwrap(await commands.sparseState(repo));
}

export async function sparseFolders(repo: RepoId, dir: string) {
  return unwrap(await commands.sparseFolders(repo, dir));
}

export async function sparsePreview(repo: RepoId, change: SparseChange) {
  return unwrap(await commands.sparsePreview(repo, change));
}

/** A turn of the repository's lane, which the footer can cancel. */
export async function sparseApply(repo: RepoId, change: SparseChange) {
  unwrap(await commands.sparseApply(repo, change));
}
