import type { FileStatus, StashContents } from "$lib/ipc";

/** One row of Apply Stash's file list (F-945). */
export interface StashFile {
  path: string;
  /** A rename's source: applied with it, since the rename deletes it. A copy keeps its
      source, so it has none. */
  oldPath: string | null;
  status: FileStatus;
}

/** Every file the stash would change, once each: its working-tree side first, then what
    only its index holds, then the untracked files of `^3`. */
export function stashFiles(contents: StashContents): StashFile[] {
  const seen = new Set<string>();
  const files: StashFile[] = [];
  const untracked = contents.untracked.map((entry) => ({ ...entry, status: "untracked" as const }));
  for (const entry of [...contents.worktree, ...contents.index, ...untracked]) {
    if (seen.has(entry.path)) continue;
    seen.add(entry.path);
    const oldPath = entry.status === "renamed" ? (entry.oldPath ?? null) : null;
    files.push({ path: entry.path, oldPath, status: entry.status });
  }
  return files.sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
}

/** The paths to apply, or `null` when every file is ticked: then it is a plain `stash
    apply`/`pop`. A rename brings its source along. */
export function pickedPaths(files: readonly StashFile[], unticked: ReadonlySet<string>): string[] | null {
  if (files.every((file) => !unticked.has(file.path))) return null;
  const paths = new Set<string>();
  for (const file of files) {
    if (unticked.has(file.path)) continue;
    paths.add(file.path);
    if (file.oldPath) paths.add(file.oldPath);
  }
  return [...paths];
}

/** The short tag beside a path in the list. */
export function stashFileTag(status: FileStatus): string {
  switch (status) {
    case "added":
      return "A";
    case "deleted":
      return "D";
    case "renamed":
      return "R";
    case "copied":
      return "C";
    case "untracked":
      return "?";
    default:
      return "M";
  }
}
