import { commands } from "./bindings";
import type {
  GitError,
  LfsOp,
  RepoId,
  RepoSetting,
  RepoSettingChange,
  SubmoduleOp,
  SubtreeOp,
} from "./bindings";
import { CogitError } from "./index";

export type { LfsOp, RepoSetting, RepoSettingChange, SubmoduleOp, SubtreeOp };

function unwrap<T>(result: { status: "ok"; data: T } | { status: "error"; error: GitError }): T {
  if (result.status === "error") throw new CogitError(result.error);
  return result.data;
}

/** Empty `paths` means every submodule, for Initialize and Synchronize only. */
export async function submoduleOp(repo: RepoId, op: SubmoduleOp, paths: string[]) {
  unwrap(await commands.submoduleOp(repo, op, paths));
}

export async function addSubmodule(repo: RepoId, url: string, path: string, branch: string | null) {
  unwrap(await commands.addSubmodule(repo, url, path, branch));
}

export async function subtreeOp(repo: RepoId, op: SubtreeOp) {
  unwrap(await commands.subtreeOp(repo, op));
}

export async function subtreePrefixes(repo: RepoId) {
  return unwrap(await commands.subtreePrefixes(repo));
}

export async function lfsVersion() {
  return unwrap(await commands.lfsVersion());
}

export async function lfsOp(repo: RepoId, op: LfsOp) {
  unwrap(await commands.lfsOp(repo, op));
}

export async function repoSettings(repo: RepoId) {
  return unwrap(await commands.repoSettings(repo));
}

export async function writeRepoSettings(repo: RepoId, changes: RepoSettingChange[]) {
  unwrap(await commands.writeRepoSettings(repo, changes));
}
