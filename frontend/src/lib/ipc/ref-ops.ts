import { Channel } from "@tauri-apps/api/core";
import { commands, type RepoId, type ResetMode } from "./bindings";
import { unwrap } from "./index";

export type { ResetMode } from "./bindings";

/** The commands behind the graph and Branches context menus (doc/04-ipc-contract.md). */
export async function resetTo(repo: RepoId, rev: string, mode: ResetMode) {
  return unwrap(await commands.resetTo(repo, rev, mode));
}

export async function isAncestor(repo: RepoId, ancestor: string, descendant: string) {
  return unwrap(await commands.isAncestor(repo, ancestor, descendant));
}

export async function compareFiles(repo: RepoId, from: string, to: string) {
  return unwrap(await commands.compareFiles(repo, from, to));
}

/** Why a new tag cannot have this name, or null. Runs `git check-ref-format`. */
export async function tagNameProblem(repo: RepoId, name: string) {
  return unwrap(await commands.tagNameProblem(repo, name));
}

export async function tagMessage(repo: RepoId, name: string) {
  return unwrap(await commands.tagMessage(repo, name));
}

export async function renameTag(repo: RepoId, from: string, to: string) {
  return unwrap(await commands.renameTag(repo, from, to));
}

export async function renameStash(repo: RepoId, index: number, message: string) {
  return unwrap(await commands.renameStash(repo, index, message));
}

export async function editAuthor(repo: RepoId, rev: string, name: string, email: string) {
  return unwrap(await commands.editAuthor(repo, rev, name, email));
}

export async function pushTo(
  repo: RepoId,
  remote: string,
  refspec: string,
  track: boolean,
  onLine: (line: string) => void,
) {
  const channel = new Channel<string>();
  channel.onmessage = onLine;
  return unwrap(await commands.pushTo(repo, remote, refspec, track, channel));
}
