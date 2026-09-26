import type { DiffRow, Hunk } from "./ipc/bindings";
import { pairRows, type SidePair } from "./diff-rows";
import { lineKey } from "./selection";

/**
 * What the Diff panel draws instead of `@@` headers (#16): rows, and between them gaps
 * that say how many lines are hidden and can be opened a piece at a time.
 *
 * Two kinds of gap. Between the hunks the backend sent, the lines are simply not here;
 * opening one asks for the whole file. Once the whole file is here, the same folds are
 * made on this side, around each change, minus whatever the user opened.
 */

/** Old-side line numbers, both ends included. */
export interface LineRange {
  from: number;
  to: number;
}

export interface Gap {
  oldFrom: number;
  oldTo: number;
  /** Where the same lines start on the new side. */
  newFrom: number;
  hidden: number;
  /** The text is on hand, so opening the gap needs no new diff. */
  loaded: boolean;
  /** A block below: ▲ opens the lines just above it. */
  up: boolean;
  /** A block above: ▼ opens the lines just below it. */
  down: boolean;
  /** The declaration the lines below the gap sit in, as `git diff` names a hunk. */
  context: string | null;
}

export type FoldEntry = { kind: "gap"; gap: Gap } | { kind: "row"; row: DiffRow; block: number };

export type SplitEntry = { kind: "gap"; gap: Gap } | { kind: "pair"; pair: SidePair; block: number };

export interface FoldInput {
  hunks: readonly Hunk[];
  oldTotal: number;
  newTotal: number;
  /** Lines kept around each change when the diff carries more than that. */
  context: number;
  revealed: readonly LineRange[];
}

/** Must match `SHOWN_GAP` in `diff_engine`: a fold this small is shown, not folded. */
const SHOWN_GAP = 2;
const MAX_CONTEXT = 80;
const STEP = 20;

/** Must match `ACCESS_SPECIFIERS` in `diff_engine/src/headers.rs`. */
const ACCESS_SPECIFIERS = new Set([
  "public:",
  "private:",
  "protected:",
  "signals:",
  "public slots:",
  "private slots:",
  "protected slots:",
]);

/** Git's own rule, the same one the backend names hunks by (R-28). */
function declares(line: string): boolean {
  return /^[\p{L}_$]/u.test(line) && !ACCESS_SPECIFIERS.has(line.trimEnd());
}

function headerContext(header: string): string | null {
  const text = header.replace(/^@@[^@]*@@ ?/, "");
  return text === "" ? null : text;
}

function isChange(row: DiffRow): boolean {
  return row.kind === "delete" || row.kind === "insert";
}

function inside(line: number, ranges: readonly LineRange[]): boolean {
  return ranges.some((range) => line >= range.from && line <= range.to);
}

/** The first line past a hunk on one side. A side with no lines starts at the line it
    follows, as git writes it (`-5,0`), so the next one is the line after that. */
function after(start: number, lines: number): number {
  return lines === 0 ? start + 1 : start + lines;
}

/** The last line before a hunk on its old side. */
function before(hunk: Hunk): number {
  return hunk.oldLines === 0 ? hunk.oldStart : hunk.oldStart - 1;
}

export function foldDiff(input: FoldInput): FoldEntry[] {
  const { hunks, oldTotal, newTotal, context, revealed } = input;
  const out: FoldEntry[] = [];
  let block = 0;
  let declared: string | null = null;
  const bare = oldTotal === 0 || newTotal === 0;

  const gap = (fields: Omit<Gap, "hidden">) => {
    if (bare || fields.oldTo < fields.oldFrom) return;
    if (out.length > 0) block += 1;
    out.push({ kind: "gap", gap: { ...fields, hidden: fields.oldTo - fields.oldFrom + 1 } });
  };

  hunks.forEach((hunk, index) => {
    const previous = hunks[index - 1];
    gap({
      oldFrom: previous ? after(previous.oldStart, previous.oldLines) : 1,
      oldTo: before(hunk),
      newFrom: previous ? after(previous.newStart, previous.newLines) : 1,
      loaded: false,
      up: true,
      down: previous !== undefined,
      context: headerContext(hunk.header),
    });

    const rows = hunk.rows;
    const firstChange = rows.findIndex(isChange);
    const lastChange = rows.findLastIndex(isChange);
    let at = 0;
    while (at < rows.length) {
      const row = rows[at]!;
      if (row.kind !== "context") {
        if (row.kind !== "collapsed") out.push({ kind: "row", row, block });
        if (row.kind === "delete" && declares(row.text)) declared = trimmed(row.text);
        at += 1;
        continue;
      }

      let end = at;
      while (end < rows.length && rows[end]!.kind === "context") end += 1;
      const run = rows.slice(at, end) as Extract<DiffRow, { kind: "context" }>[];
      const before = firstChange !== -1 && firstChange < at;
      const after = lastChange >= end;
      const keep = run.map(
        (cell, offset) =>
          (before && offset < context) ||
          (after && offset >= run.length - context) ||
          inside(cell.old, revealed),
      );

      let offset = 0;
      while (offset < run.length) {
        if (keep[offset]) {
          const cell = run[offset]!;
          if (declares(cell.text)) declared = trimmed(cell.text);
          out.push({ kind: "row", row: cell, block });
          offset += 1;
          continue;
        }
        let stop = offset;
        while (stop < run.length && !keep[stop]) stop += 1;
        const hidden = run.slice(offset, stop);
        for (const cell of hidden) if (declares(cell.text)) declared = trimmed(cell.text);
        if (hidden.length <= SHOWN_GAP) {
          for (const cell of hidden) out.push({ kind: "row", row: cell, block });
        } else {
          const first = hidden[0]!;
          gap({
            oldFrom: first.old,
            oldTo: hidden[hidden.length - 1]!.old,
            newFrom: first.new,
            loaded: true,
            up: stop < run.length || after,
            down: offset > 0 || before,
            context: declared,
          });
        }
        offset = stop;
      }
      at = end;
    }
  });

  const last = hunks[hunks.length - 1];
  if (last) {
    gap({
      oldFrom: after(last.oldStart, last.oldLines),
      oldTo: oldTotal,
      newFrom: after(last.newStart, last.newLines),
      loaded: false,
      up: false,
      down: true,
      context: null,
    });
  }
  return out;
}

function trimmed(line: string): string {
  return [...line.trimEnd()].slice(0, MAX_CONTEXT).join("");
}

/** ▲ opens the lines just above the change below, ▼ those just below the change above. */
export function revealRange(gap: Gap, how: "up" | "down" | "all"): LineRange {
  if (how === "up") return { from: Math.max(gap.oldFrom, gap.oldTo - STEP + 1), to: gap.oldTo };
  if (how === "down") return { from: gap.oldFrom, to: Math.min(gap.oldTo, gap.oldFrom + STEP - 1) };
  return { from: gap.oldFrom, to: gap.oldTo };
}

/** Side by side pairs each block on its own; a gap spans both halves. */
export function splitRows(entries: readonly FoldEntry[]): SplitEntry[] {
  const out: SplitEntry[] = [];
  let rows: DiffRow[] = [];
  let block = 0;
  const flush = () => {
    for (const pair of pairRows(rows)) out.push({ kind: "pair", pair, block });
    rows = [];
  };
  for (const entry of entries) {
    if (entry.kind === "gap") {
      flush();
      out.push(entry);
    } else {
      if (entry.block !== block) flush();
      block = entry.block;
      rows.push(entry.row);
    }
  }
  flush();
  return out;
}

/** The changed lines of one block, for its Stage, Unstage, Discard and Select. */
export function blockKeys(entries: readonly FoldEntry[], block: number): Set<string> {
  const keys = new Set<string>();
  for (const entry of entries) {
    if (entry.kind !== "row" || entry.block !== block) continue;
    const key = lineKey(entry.row);
    if (key) keys.add(key);
  }
  return keys;
}

/** Where each run of changed rows starts: what F6 and the arrows step between (#13). */
export function changeStarts(changed: readonly boolean[]): number[] {
  const starts: number[] = [];
  changed.forEach((on, index) => {
    if (on && !changed[index - 1]) starts.push(index);
  });
  return starts;
}

/**
 * The change the view is on: the last one starting at or above `top + lead`, the row a
 * jump puts it on. At the very top the first change counts while it is on screen.
 * `-1` while the view is above the first change.
 */
export function changeAt(starts: readonly number[], top: number, bottom: number, lead: number): number {
  let at = -1;
  starts.forEach((start, index) => {
    if (start <= top + lead) at = index;
  });
  if (at === -1 && top === 0 && (starts[0] ?? Infinity) <= bottom) at = 0;
  return at;
}

export function navState(count: number, current: number): { prev: boolean; next: boolean } {
  return { prev: current > 0, next: current < count - 1 };
}
