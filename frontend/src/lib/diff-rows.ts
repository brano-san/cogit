import type { DiffRow } from "$lib/ipc";

export type SideKind = "context" | "delete" | "insert";

export interface SideCell {
  kind: SideKind;
  line: number;
  text: string;
  inline: [number, number][];
  moved: boolean;
  /** Both ends of one move share it, which is what ties the two sides together. */
  moveId: number | null;
  /** The file ends on this line without a newline (`\ No newline at end of file`). */
  noNewline: boolean;
}

export interface SidePair {
  left: SideCell | null;
  right: SideCell | null;
}

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
          moveId: row.moveId ?? null,
          noNewline: row.noNewline ?? false,
        });
        break;
      case "insert":
        inserts.push({
          kind: "insert",
          line: row.new,
          text: row.text,
          inline: row.inline,
          moved: row.moved ?? false,
          moveId: row.moveId ?? null,
          noNewline: row.noNewline ?? false,
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
          moveId: null,
          noNewline: row.noNewline ?? false,
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

/** The selection key of a side-by-side cell: the same `d:`/`i:` key its row has in Unified. */
export function cellKey(cell: SideCell | null): string | null {
  if (cell?.kind === "delete") return `d:${cell.line}`;
  if (cell?.kind === "insert") return `i:${cell.line}`;
  return null;
}

/** A side-by-side row is marked when either of its cells is selected. */
export function pairPicked(pair: SidePair, selected: ReadonlySet<string>): boolean {
  return [pair.left, pair.right].some((cell) => {
    const key = cellKey(cell);
    return key !== null && selected.has(key);
  });
}

export interface Segment {
  text: string;
  changed: boolean;
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

/** One ribbon in the band between the columns: which rows on the left face which on the right. */
export interface Connector {
  fromTop: number;
  fromBottom: number;
  toTop: number;
  toBottom: number;
  /** A move is drawn dashed: its two ends are usually nowhere near each other. */
  moved: boolean;
}

/** A row of the rendered side-by-side list; `null` is a fold, which connects nothing. */
export type ConnectorRow = SidePair | null;

function changed(row: ConnectorRow): boolean {
  return row?.left?.kind === "delete" || row?.right?.kind === "insert";
}

/**
 * What to draw in the band between the two columns.
 *
 * Two kinds. A move is matched by `moveId`, so its ends join however far apart they sit.
 * Everything else is a run of consecutive changed rows facing the rows opposite it —
 * short, because `pairRows` has already lined the two sides up (R-105).
 */
export function connectors(rows: readonly ConnectorRow[]): Connector[] {
  const ends = new Map<number, { left: number[]; right: number[] }>();
  rows.forEach((row, index) => {
    for (const [side, cell] of [
      ["left", row?.left],
      ["right", row?.right],
    ] as const) {
      if (!cell || cell.moveId === null) continue;
      const pair = ends.get(cell.moveId) ?? { left: [], right: [] };
      pair[side].push(index);
      ends.set(cell.moveId, pair);
    }
  });

  const out: Connector[] = [];
  const claimed = new Set<number>();
  for (const { left, right } of ends.values()) {
    // One end outside the rendered rows is not a pairing anyone can see; leave it plain.
    if (left.length === 0 || right.length === 0) continue;
    out.push({
      fromTop: Math.min(...left),
      fromBottom: Math.max(...left),
      toTop: Math.min(...right),
      toBottom: Math.max(...right),
      moved: true,
    });
    for (const index of [...left, ...right]) claimed.add(index);
  }

  let start: number | null = null;
  rows.forEach((row, index) => {
    const open = changed(row) && !claimed.has(index);
    if (open && start === null) start = index;
    if (!open && start !== null) {
      out.push({ fromTop: start, fromBottom: index - 1, toTop: start, toBottom: index - 1, moved: false });
      start = null;
    }
  });
  if (start !== null) {
    const last = rows.length - 1;
    out.push({ fromTop: start, fromBottom: last, toTop: start, toBottom: last, moved: false });
  }

  return out.sort((a, b) => a.fromTop - b.fromTop);
}

const EXPAND_BY = 20;
const WHOLE_FILE = 100_000;

export function expandedContext(current: number, whole: boolean): number {
  return whole ? WHOLE_FILE : current + EXPAND_BY;
}
