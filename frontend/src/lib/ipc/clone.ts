import { Channel } from "@tauri-apps/api/core";
import { commands } from "./bindings";
import { unwrap } from "./index";

export type { CloneDestination, CloneRequest, Login, RemoteBranches } from "./bindings";
import type { CloneRequest, Login } from "./bindings";

/** Repository ▸ Clone… (F-575). The check behind Next: `git ls-remote`; the credential helper may ask. */
export async function remoteBranches(source: string, login: Login | null = null) {
  return unwrap(await commands.remoteBranches(source, login));
}

export async function cloneDestination(path: string) {
  return unwrap(await commands.cloneDestination(path));
}

/** The clipboard's text only when it is a repository URL, `null` otherwise. */
export async function clipboardRepositoryUrl() {
  return unwrap(await commands.clipboardRepositoryUrl());
}

/** The new repository's root once it is cloned; a queue operation of kind `clone`. */
export async function cloneRepository(request: CloneRequest, login: Login | null, onLine: (line: string) => void) {
  const channel = new Channel<string>();
  channel.onmessage = onLine;
  return unwrap(await commands.cloneRepository(request, login, channel));
}
