import type { Hunk } from "$lib/ipc";

/**
 * What a selected line still means once the diff under it is replaced.
 *
 * Keys are line numbers of one diff (`d:` old side, `i:` new side). Staging a block moves
 * the index under every deletion below it, and the same number then names another line.
 * A key is kept only where the new diff has the same line at the same number, facing the
 * same place on the other side; every other key is dropped rather than guessed at.
 */
export function keepSelection(
  selected: ReadonlySet<string>,
  before: readonly Hunk[],
  after: readonly Hunk[],
): Set<string> {
  if (selected.size === 0) return new Set();
  const was = lines(before);
  const now = lines(after);
  return new Set([...selected].filter((key) => was.has(key) && was.get(key) === now.get(key)));
}

/** Each changed line's text and where it sits on the other side, by its key. */
function lines(hunks: readonly Hunk[]): Map<string, string> {
  const out = new Map<string, string>();
  // Lines of each side up to the last row seen; unchanged lines pair one to one.
  let old = 0;
  let nw = 0;
  for (const hunk of hunks) {
    for (const row of hunk.rows) {
      if (row.kind === "context") {
        old = row.old;
        nw = row.new;
      } else if (row.kind === "delete") {
        nw += row.old - 1 - old;
        old = row.old;
        out.set(`d:${row.old}`, `${nw}\u0000${row.text}`);
      } else if (row.kind === "insert") {
        old += row.new - 1 - nw;
        nw = row.new;
        out.set(`i:${row.new}`, `${old}\u0000${row.text}`);
      }
    }
  }
  return out;
}
