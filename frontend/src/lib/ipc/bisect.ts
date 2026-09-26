import { commands, type BisectMark, type RepoId } from "./bindings";
import { unwrap } from "./index";

export type { BisectMark, BisectState } from "./bindings";

/** `git bisect` (doc/04-ipc-contract.md): each step checks out the next commit to test. */
export async function bisectStart(repo: RepoId, bad: string, good: string | null) {
  return unwrap(await commands.bisectStart(repo, bad, good));
}

/** `rev` null marks HEAD, the commit git checked out. */
export async function bisectMark(repo: RepoId, mark: BisectMark, rev: string | null) {
  return unwrap(await commands.bisectMark(repo, mark, rev));
}

export async function bisectReset(repo: RepoId) {
  return unwrap(await commands.bisectReset(repo));
}
