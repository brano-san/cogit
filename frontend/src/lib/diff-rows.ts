export interface SearchHit {
  /** Row index in the list being rendered, so the view can scroll straight to it. */
  index: number;
  /** Which column holds the hit; unified rows are all `left`. */
  side: "left" | "right";
  from: number;
  to: number;
}

/** Two texts per row: side by side has two columns, unified leaves the second `null`. */
export type SearchRow = readonly [string | null, string | null];

/**
 * Case-insensitive plain-text search over the rows as rendered.
 *
 * Plain text: the query is escaped, so a stray `(` in a search box finds a bracket
 * rather than throwing. Matches do not overlap — `aa` in `aaaa` is two hits, not three.
 * Matched in the line as written: lowering it first can change its length ("İ" becomes
 * two code units), and the offsets would mark the wrong characters.
 */
export function searchRows(rows: readonly SearchRow[], query: string): SearchHit[] {
  const needle = query.trim();
  if (needle.length === 0) return [];
  const pattern = new RegExp(needle.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "giu");

  const hits: SearchHit[] = [];
  rows.forEach((row, index) => {
    (["left", "right"] as const).forEach((side, column) => {
      const text = row[column];
      if (text === null || text === undefined) return;
      for (const match of text.matchAll(pattern)) {
        hits.push({ index, side, from: match.index, to: match.index + match[0].length });
      }
    });
  });
  return hits;
}

/** Next or previous hit, wrapping at both ends. `-1` when there is nothing to step to. */
export function stepHit(hits: readonly unknown[], current: number, delta: number): number {
  if (hits.length === 0) return -1;
  return (current + delta + hits.length) % hits.length;
}

const EXPAND_BY = 20;
const WHOLE_FILE = 100_000;

export function expandedContext(current: number, whole: boolean): number {
  return whole ? WHOLE_FILE : current + EXPAND_BY;
}
