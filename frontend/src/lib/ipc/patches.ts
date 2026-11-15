import { commands, type PatchMode, type PatchPreview, type RepoId } from "./bindings";
import { unwrap } from "./index";

export type { PatchPreview, PatchStat } from "./bindings";

/** Patches (F-948, doc/04-ipc-contract.md, «Patches»). */
export async function saveCommitPatches(repo: RepoId, commits: string[], target: string, oneFile: boolean) {
  return unwrap(await commands.saveCommitPatches(repo, commits, target, oneFile));
}

export async function commitPatchText(repo: RepoId, commits: string[]): Promise<string> {
  return unwrap(await commands.commitPatchText(repo, commits));
}

export async function saveWorktreePatch(repo: RepoId, paths: string[], staged: boolean, target: string) {
  return unwrap(await commands.saveWorktreePatch(repo, paths, staged, target));
}

export async function worktreePatchText(repo: RepoId, paths: string[], staged: boolean): Promise<string> {
  return unwrap(await commands.worktreePatchText(repo, paths, staged));
}

export async function readPatchFile(path: string): Promise<string> {
  return unwrap(await commands.readPatchFile(path));
}

/** The clipboard's text, only when it holds a patch. */
export async function clipboardPatch(): Promise<string | null> {
  return unwrap(await commands.clipboardPatch());
}

export async function patchPreview(repo: RepoId, text: string, mode: PatchMode): Promise<PatchPreview> {
  return unwrap(await commands.patchPreview(repo, text, mode));
}

export async function applyPatch(repo: RepoId, text: string, mode: PatchMode) {
  return unwrap(await commands.applyPatch(repo, text, mode));
}
