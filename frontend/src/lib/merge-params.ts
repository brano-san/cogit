import type { RepoId } from "./ipc";
import { asCogitError, commandReport } from "./notices";

export interface MergeRequest {
  repo: RepoId;
  path: string;
}

/** In the URL, not in shared state: the window has to survive a webview reload (T2.5). */
export function mergeUrl(repo: RepoId, path: string): string {
  return `merge.html?${new URLSearchParams({ repo: String(repo), path }).toString()}`;
}

export function parseMerge(search: string): MergeRequest | null {
  const params = new URLSearchParams(search);
  const repo = Number(params.get("repo"));
  const path = params.get("path");
  if (!Number.isInteger(repo) || repo <= 0 || !path) return null;
  return { repo, path };
}

/** What the window says when a step fails: git's own record in full for a command (INV-05),
    the message for anything else. */
export function failureText(err: unknown): string {
  const error = asCogitError(err);
  if (!error) return "";
  return error.detail.kind === "command" ? commandReport(error.detail.data) : error.message;
}
