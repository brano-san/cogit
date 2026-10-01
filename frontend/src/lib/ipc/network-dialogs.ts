import { Channel } from "@tauri-apps/api/core";
import { commands, type FetchOptions, type NetworkDefaults, type PullOptions, type PushOptions, type RepoId } from "./bindings";
import { unwrap } from "./index";

export type {
  FetchOptions,
  NetworkDefaults,
  NotesFetch,
  PullMethod,
  PullOptions,
  PushCommit,
  PushOptions,
  PushOutcome,
  PushPreview,
  TagsMode,
} from "./bindings";

function lines(onLine: (line: string) => void): Channel<string> {
  const channel = new Channel<string>();
  channel.onmessage = onLine;
  return channel;
}

export async function fetchWith(repo: RepoId, remote: string, options: FetchOptions, onLine: (line: string) => void) {
  return unwrap(await commands.fetchWith(repo, remote, options, lines(onLine)));
}

export async function pullWith(repo: RepoId, remote: string, options: PullOptions, onLine: (line: string) => void) {
  return unwrap(await commands.pullWith(repo, remote, options, lines(onLine)));
}

export async function pushWith(repo: RepoId, options: PushOptions, onLine: (line: string) => void) {
  return unwrap(await commands.pushWith(repo, options, lines(onLine)));
}

export async function pushNotes(repo: RepoId, remote: string, onLine: (line: string) => void) {
  return unwrap(await commands.pushNotes(repo, remote, lines(onLine)));
}

export async function mergeNotes(repo: RepoId, remote: string, namespace: string) {
  unwrap(await commands.mergeNotes(repo, remote, namespace));
}

export async function pushPreview(repo: RepoId, local: string, remote: string, branch: string, limit: number) {
  return unwrap(await commands.pushPreview(repo, local, remote, branch, limit));
}

export async function networkDefaults(repo: RepoId): Promise<NetworkDefaults> {
  return unwrap(await commands.networkDefaults(repo));
}

export async function saveNetworkDefaults(repo: RepoId, defaults: NetworkDefaults) {
  unwrap(await commands.saveNetworkDefaults(repo, defaults));
}
