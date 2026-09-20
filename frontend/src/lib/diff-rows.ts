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
