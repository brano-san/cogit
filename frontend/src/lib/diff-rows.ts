import type { DiffRow, Hunk } from "$lib/ipc";

export type SideKind = "context" | "delete" | "insert";

export interface SideCell {
  kind: SideKind;
  line: number;
  text: string;
  inline: [number, number][];
  moved: boolean;
}

export interface SidePair {
  left: SideCell | null;
  right: SideCell | null;
}

export type FlatEntry =
  | { kind: "header"; hunk: number; text: string }
  | { kind: "row"; hunk: number; row: DiffRow };

/** Deletions and the insertions that replace them line up; a longer side pads the other. */
export function pairRows(rows: readonly DiffRow[]): SidePair[] {
  const pairs: SidePair[] = [];
  let deletes: SideCell[] = [];
  let inserts: SideCell[] = [];

  const flushBlock = () => {
    const height = Math.max(deletes.length, inserts.length);
    for (let i = 0; i < height; i++) {
      pairs.push({ left: deletes[i] ?? null, right: inserts[i] ?? null });
    }
    deletes = [];
    inserts = [];
  };

  for (const row of rows) {
    switch (row.kind) {
      case "delete":
        // An insertion already seen belongs to the previous block, not to this deletion.
        if (inserts.length > 0) flushBlock();
        deletes.push({
          kind: "delete",
          line: row.old,
          text: row.text,
          inline: row.inline,
          moved: row.moved ?? false,
        });
        break;
      case "insert":
        inserts.push({
          kind: "insert",
          line: row.new,
          text: row.text,
          inline: row.inline,
          moved: row.moved ?? false,
        });
        break;
      case "context": {
        flushBlock();
        const cell: SideCell = {
          kind: "context",
          line: 0,
          text: row.text,
          inline: [],
          moved: false,
        };
        pairs.push({
          left: { ...cell, line: row.old },
          right: { ...cell, line: row.new },
        });
        break;
      }
      case "collapsed":
        flushBlock();
        break;
    }
  }
  flushBlock();
  return pairs;
}

export function flatten(hunks: readonly Hunk[]): FlatEntry[] {
  const entries: FlatEntry[] = [];
  hunks.forEach((hunk, index) => {
    entries.push({ kind: "header", hunk: index, text: hunk.header });
    for (const row of hunk.rows) {
      entries.push({ kind: "row", hunk: index, row });
    }
  });
  return entries;
}

export interface Segment {
  text: string;
  changed: boolean;
}

/** Spans are UTF-16 offsets, which is exactly what `String.prototype.slice` indexes by. */
export function segments(text: string, spans: readonly [number, number][]): Segment[] {
  if (spans.length === 0) return [{ text, changed: false }];

  const out: Segment[] = [];
  let at = 0;
  for (const [from, to] of spans) {
    const start = Math.max(at, Math.min(from, text.length));
    const end = Math.max(start, Math.min(to, text.length));
    if (start > at) out.push({ text: text.slice(at, start), changed: false });
    if (end > start) out.push({ text: text.slice(start, end), changed: true });
    at = end;
  }
  if (at < text.length) out.push({ text: text.slice(at), changed: false });
  return out;
}

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
 * Plain text, not a regular expression: a stray `(` in a search box should find a
 * bracket, not throw. Matches do not overlap — `aa` in `aaaa` is two hits, not three.
 */
export function searchRows(rows: readonly SearchRow[], query: string): SearchHit[] {
  const needle = query.trim().toLowerCase();
  if (needle.length === 0) return [];

  const hits: SearchHit[] = [];
  rows.forEach((row, index) => {
    (["left", "right"] as const).forEach((side, column) => {
      const text = row[column];
      if (text === null || text === undefined) return;
      const haystack = text.toLowerCase();
      let at = haystack.indexOf(needle);
      while (at !== -1) {
        hits.push({ index, side, from: at, to: at + needle.length });
        at = haystack.indexOf(needle, at + needle.length);
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

/** How many lines the diff is not showing between two hunks. */
export function gapBetween(previous: Hunk | null, next: Hunk): number {
  const from = previous === null ? 1 : previous.oldStart + previous.oldLines;
  return Math.max(next.oldStart - from, 0);
}

const EXPAND_BY = 20;
const WHOLE_FILE = 100_000;

export function expandedContext(current: number, whole: boolean): number {
  return whole ? WHOLE_FILE : current + EXPAND_BY;
}
