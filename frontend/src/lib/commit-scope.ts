import { matchesMask } from "$lib/files";
import type { FileEntry } from "$lib/ipc";

export interface CommitScope {
  label: string;
  hidden: number;
  paths: string[] | null;
  warning: string | null;
}

export function commitScope(staged: readonly FileEntry[], mask: string): CommitScope {
  const shown = staged.filter((file) => matchesMask(file.path, mask));
  const hidden = staged.length - shown.length;

  if (hidden === 0) {
    return { label: `Commit ${staged.length}`, hidden: 0, paths: null, warning: null };
  }
  return {
    label: `Commit ${shown.length} shown`,
    hidden,
    paths: shown.map((file) => file.path),
    warning:
      hidden === 1
        ? "1 more changed file is hidden by the filter"
        : `${hidden} more changed files are hidden by the filter`,
  };
}
