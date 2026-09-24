import { matchesMask } from "$lib/files";
import type { FileEntry } from "$lib/ipc";

export interface CommitScope {
  label: string;
  hidden: number;
  paths: string[] | null;
  warning: string | null;
  /** Every staged file is filtered out: an empty path list would commit them all. */
  empty: boolean;
}

/** `visible` is the staged rows the list shows, when it said; the glob of `mask` is only
    a guess at them, and the list filters by far more (regex, content, status, switches). */
export function commitScope(
  staged: readonly FileEntry[],
  mask: string,
  visible: readonly string[] | null = null,
): CommitScope {
  // A set, not `includes`: staging two thousand files made this 4 million comparisons,
  // run on every read-back of the list (doc/12-risks.md, R-310).
  const seen = visible === null ? null : new Set(visible);
  const shown = staged.filter((file) => (seen === null ? matchesMask(file.path, mask) : seen.has(file.path)));
  const hidden = staged.length - shown.length;

  if (hidden === 0) {
    return { label: `Commit ${staged.length}`, hidden: 0, paths: null, warning: null, empty: false };
  }
  return {
    label: `Commit ${shown.length} shown`,
    hidden,
    empty: shown.length === 0,
    paths: shown.map((file) => file.path),
    warning:
      hidden === 1
        ? "1 more changed file is hidden by the filter"
        : `${hidden} more changed files are hidden by the filter`,
  };
}
