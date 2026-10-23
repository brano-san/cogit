import type { TagRequest } from "./ipc";
import { refNameProblem } from "./names";
import { splitUpstream } from "./push-to";

/** Shown under the field while typing; the create buttons stay off while there is one. */
export function tagNameHint(name: string, taken: readonly string[]): string | null {
  const trimmed = name.trim();
  if (trimmed === "") return "Enter a name.";
  if (taken.includes(trimmed)) return `A tag named '${trimmed}' already exists.`;
  return refNameProblem(trimmed);
}

export function tagRequest(name: string, message: string, target: string): TagRequest {
  const text = message.replace(/\s+$/, "");
  return { name: name.trim(), target, message: text.trim() === "" ? null : text, force: false };
}

/** Where Add Tag & Push sends the tag: the only remote, else the current branch's upstream
    remote, else origin; null when nothing decides it and the dialog asks. */
export function tagPushRemote(remotes: readonly string[], upstream: string | null): string | null {
  if (remotes.length === 1) return remotes[0] ?? null;
  const tracked = upstream ? splitUpstream(upstream, remotes) : null;
  return tracked?.remote ?? (remotes.includes("origin") ? "origin" : null);
}
