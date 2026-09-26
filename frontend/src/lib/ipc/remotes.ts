import { Channel } from "@tauri-apps/api/core";
import { commands, type RemoteInfo, type RepoId } from "./bindings";
import { unwrap } from "./index";

export type { RemoteInfo } from "./bindings";

/** The menu of a remote in Branches (doc/04-ipc-contract.md, «Remotes»). */
export async function remoteInfo(repo: RepoId, name: string): Promise<RemoteInfo> {
  return unwrap(await commands.remoteInfo(repo, name));
}

export async function renameRemote(repo: RepoId, from: string, to: string) {
  return unwrap(await commands.renameRemote(repo, from, to));
}

export async function removeRemote(repo: RepoId, name: string) {
  return unwrap(await commands.removeRemote(repo, name));
}

export async function setRemoteProperties(repo: RepoId, name: string, url: string, backgroundFetch: boolean) {
  return unwrap(await commands.setRemoteProperties(repo, name, url, backgroundFetch));
}

function lines(onLine: (line: string) => void): Channel<string> {
  const channel = new Channel<string>();
  channel.onmessage = onLine;
  return channel;
}

/** `false`: nothing new came. */
export async function fetchMore(repo: RepoId, remote: string, onLine: (line: string) => void) {
  return unwrap(await commands.fetchMore(repo, remote, lines(onLine)));
}

export async function fetchDepth(repo: RepoId, remote: string, depth: number, onLine: (line: string) => void) {
  return unwrap(await commands.fetchDepth(repo, remote, depth, lines(onLine)));
}
