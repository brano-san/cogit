import type { FileEntry } from "$lib/ipc";

/** What `git add --all` stages exactly as it would the path. An ignored, skipped or
    unchanged row is listed only because a view switch asked for it: named, git refuses
    or skips it, and `--all` would pass it by without a word. */
const CHANGES = new Set<FileEntry["status"]>([
  "added",
  "modified",
  "deleted",
  "renamed",
  "copied",
  "untracked",
  "conflicted",
]);

/** Whether staging `paths` stages every change in Unstaged, so that one `git add --all`
    does it without a path list (doc/12-risks.md, R-311). */
export function stagesEverything(paths: readonly string[], unstaged: readonly FileEntry[]): boolean {
  if (paths.length === 0 || paths.length !== unstaged.length) return false;
  if (!unstaged.every((file) => CHANGES.has(file.status))) return false;
  const picked = new Set(paths);
  return picked.size === unstaged.length && unstaged.every((file) => picked.has(file.path));
}
