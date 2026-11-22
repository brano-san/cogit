import { fileState, sideOfRow, type StateSide } from "./file-state";
import { containsFilter, ignoreFilterCase, type FilterCase } from "./filter-case";
import { fileName, globToRegExp } from "./files";
import type { ListFile } from "./file-view";

/**
 * One field to search them all: the name, the path and the state at once, because a
 * switch asking which one you meant is a question the app can answer itself (issue 10).
 */

export interface Pattern {
  /** Ready to test, or `null` when the text is empty or the expression is broken. */
  test: ((subject: string) => boolean) | null;
  /** True when regex mode is on and the text does not compile. */
  broken: boolean;
  /** A literal substring in the basename is also in the full path. Anchored regexes
      and globs must still test the two separately. */
  pathIncludesName?: boolean;
}

export function compile(text: string, regex: boolean, mode: FilterCase = "ignore"): Pattern {
  const needle = text.trim();
  if (needle === "") return { test: null, broken: false };

  if (regex) {
    try {
      const expression = new RegExp(needle, ignoreFilterCase(needle, mode) ? "i" : "");
      return { test: (subject) => expression.test(subject), broken: false };
    } catch {
      // A half-typed expression is the normal state of a search box, not an error.
      return { test: null, broken: true };
    }
  }

  // A mask like `*.rs` (F-010) matches a whole name or path, not a piece of one.
  if (/[*?]/.test(needle)) {
    const folded = ignoreFilterCase(needle, mode);
    const glob = globToRegExp(folded ? needle.toLowerCase() : needle);
    return { test: (subject) => glob.test(folded ? subject.toLowerCase() : subject), broken: false };
  }
  return { test: containsFilter(needle, mode), broken: false, pathIncludesName: true };
}

/** Everything the row shows, so the filter sees what the eye sees. Each part is tested on
    its own: joined, `$` would meet the state and `^` the name instead of the path. */
export function haystack(file: ListFile, side: StateSide = "worktree"): string[] {
  const parts = [fileName(file.path), file.path, fileState(file, sideOfRow(file.indexState, side)).text];
  if (file.oldPath) parts.push(file.oldPath);
  return parts;
}

export function matches(file: ListFile, pattern: Pattern, side: StateSide = "worktree"): boolean {
  const test = pattern.test;
  if (test === null) return true;
  return (!pattern.pathIncludesName && test(fileName(file.path))) || test(file.path) ||
    test(fileState(file, sideOfRow(file.indexState, side)).text) ||
    (file.oldPath != null && file.oldPath !== "" && test(file.oldPath));
}
