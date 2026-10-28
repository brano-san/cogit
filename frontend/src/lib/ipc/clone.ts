import { Channel } from "@tauri-apps/api/core";
import { commands } from "./bindings";
import { unwrap } from "./index";

export type { CloneDestination, CloneRequest, FolderKind, RemoteBranches, SignIn } from "./bindings";
import type { CloneRequest, SignIn } from "./bindings";

/** Repository ▸ Clone… (F-575). The check behind Next: `git ls-remote`; the credential helper may ask. */
export async function remoteBranches(source: string, credential: SignIn | null = null, into: string | null = null) {
  return unwrap(await commands.remoteBranches(source, credential, into));
}

export async function cloneDestination(path: string) {
  return unwrap(await commands.cloneDestination(path));
}

/** The Welcome dialog: what a folder is, without opening it. */
export async function folderKind(path: string) {
  return unwrap(await commands.folderKind(path));
}

/** `git init` in the folder (created if missing); the root to open. */
export async function initRepository(path: string) {
  return unwrap(await commands.initRepository(path));
}

/** The clipboard's text only when it is a repository URL, `null` otherwise. */
export async function clipboardRepositoryUrl() {
  return unwrap(await commands.clipboardRepositoryUrl());
}

/** The new repository's root once it is cloned; a queue operation of kind `clone`. */
export async function cloneRepository(request: CloneRequest, onLine: (line: string) => void, credential: SignIn | null = null) {
  const channel = new Channel<string>();
  channel.onmessage = onLine;
  return unwrap(await commands.cloneRepository(request, credential, channel));
}
