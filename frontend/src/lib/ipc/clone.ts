import { Channel } from "@tauri-apps/api/core";
import { commands } from "./bindings";
import { unwrap } from "./index";

export type { CloneDestination, CloneRequest, RemoteBranches } from "./bindings";
import type { CloneRequest } from "./bindings";

/** Repository ▸ Clone… (F-575). The check behind Next: `git ls-remote`, never prompts. */
export async function remoteBranches(source: string) {
  return unwrap(await commands.remoteBranches(source));
}

export async function cloneDestination(path: string) {
  return unwrap(await commands.cloneDestination(path));
}

/** The clipboard's text only when it is a repository URL, `null` otherwise. */
export async function clipboardRepositoryUrl() {
  return unwrap(await commands.clipboardRepositoryUrl());
}

/** The new repository's root once it is cloned; a queue operation of kind `clone`. */
export async function cloneRepository(request: CloneRequest, onLine: (line: string) => void) {
  const channel = new Channel<string>();
  channel.onmessage = onLine;
  return unwrap(await commands.cloneRepository(request, channel));
}
