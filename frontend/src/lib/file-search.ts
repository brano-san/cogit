import type { FileEntry } from "./ipc/bindings";
import { fileName, statusLabel } from "./files";

/**
 * One field to search them all: the name, the path and the state at once, because a
 * switch asking which one you meant is a question the app can answer itself (issue 10).
 */

export interface Pattern {
  /** Ready to test, or `null` when the text is empty or the expression is broken. */
  test: ((subject: string) => boolean) | null;
  /** True when regex mode is on and the text does not compile. */
  broken: boolean;
}

export function compile(text: string, regex: boolean): Pattern {
  const needle = text.trim();
  if (needle === "") return { test: null, broken: false };

  if (regex) {
    try {
      const expression = new RegExp(needle, "i");
      return { test: (subject) => expression.test(subject), broken: false };
    } catch {
      // A half-typed expression is the normal state of a search box, not an error.
      return { test: null, broken: true };
    }
  }

  const lower = needle.toLowerCase();
  return { test: (subject) => subject.toLowerCase().includes(lower), broken: false };
}

/** Everything the row shows, joined, so the filter sees what the eye sees. */
export function haystack(file: FileEntry): string {
  const parts = [fileName(file.path), file.path, statusLabel(file.status)];
  if (file.oldPath) parts.push(file.oldPath);
  return parts.join(" ");
}

export function matches(file: FileEntry, pattern: Pattern): boolean {
  if (pattern.test === null) return true;
  return pattern.test(haystack(file));
}
