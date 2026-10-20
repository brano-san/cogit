import type { DiffRow, Hunk } from "./ipc/bindings";

/**
 * What the Diff panel draws instead of `@@` headers (#16): rows, and between them gaps
 * that say how many lines are hidden and can be opened a piece at a time.
 *
 * Two kinds of gap. Between the hunks the backend sent, the lines are simply not here;
 * opening one asks for the whole file. Once the whole file is here, the same folds are
 * made on this side, around each change, minus whatever the user opened.
 *
 * A gap is also the strip that carries Stage, Unstage and Discard for the block below it.
 * Where a block needs one and nothing is folded there, a gap that hides nothing stands in.
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

export interface FoldInput {
  hunks: readonly Hunk[];
  oldTotal: number;
  newTotal: number;
  /** Lines kept around each change when the diff carries more than that. */
  context: number;
  revealed: readonly LineRange[];
  /** A strip above every block of changes, folded or not: the diff can stage. A block is
      what git makes a hunk of at `context` lines; the strip sits `context` lines above it. */
  strips?: boolean;
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

/** The first line of a hunk on one side; a side with no lines starts past the line it follows. */
function firstLine(start: number, lines: number): number {
  return lines === 0 ? start + 1 : start;
}

/** The last line before a hunk on its old side. */
function before(hunk: Hunk): number {
  return hunk.oldLines === 0 ? hunk.oldStart : hunk.oldStart - 1;
}

export function foldDiff(input: FoldInput): FoldEntry[] {
  const { hunks, oldTotal, newTotal, context, revealed, strips = false } = input;
  const out: FoldEntry[] = [];
  let block = 0;
  let declared: string | null = null;
  const bare = oldTotal === 0 || newTotal === 0;
  /** A gap since the last changed line: the next block has its strip. */
  let stripped = false;

  const gap = (fields: Omit<Gap, "hidden">) => {
    if (bare || fields.oldTo < fields.oldFrom) return;
    if (out.length > 0) block += 1;
    out.push({ kind: "gap", gap: { ...fields, hidden: fields.oldTo - fields.oldFrom + 1 } });
    stripped = true;
  };
  /** A strip that hides nothing, above the lines at `oldFrom` / `newFrom`. */
  const strip = (oldFrom: number, newFrom: number) => {
    if (out.length > 0) block += 1;
    const fields = { oldFrom, oldTo: oldFrom - 1, newFrom, hidden: 0, loaded: true, up: false, down: false };
    out.push({ kind: "gap", gap: { ...fields, context: declared } });
    stripped = true;
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
        // A hunk that starts with a change and has no gap above it: at the top of the file.
        if (strips && at === firstChange && !stripped) {
          strip(firstLine(hunk.oldStart, hunk.oldLines), firstLine(hunk.newStart, hunk.newLines));
        }
        out.push({ kind: "row", row, block });
        stripped = false;
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
      // Git starts a new hunk past twice the context; a gap in the run is the strip already.
      const wanted = strips && after && (before ? run.length > 2 * context : !stripped);
      const stripAt = Math.max(0, run.length - context);

      let offset = 0;
      while (offset < run.length) {
        if (wanted && !stripped && offset === stripAt) strip(run[offset]!.old, run[offset]!.new);
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
      if (wanted && !stripped) strip(run[run.length - 1]!.old + 1, run[run.length - 1]!.new + 1);
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

/**
 * The rows to highlight. All the loaded ones while each side fits under `max`, parsed as
 * one text so a comment opened above a fold still colours the lines under it. Past that —
 * the whole file, once a fold asked for it — only the rows on show: the parser gives up on
 * a side over `max`, and the whole diff went grey the moment a band was opened. `shown`
 * is asked only then, so opening a band in a small file does not parse it again.
 */
export function highlightedRows(
  hunks: readonly Hunk[],
  shown: () => readonly FoldEntry[],
  max: number,
): DiffRow[] {
  let old = 0;
  let next = 0;
  for (const hunk of hunks) {
    old += hunk.oldLines;
    next += hunk.newLines;
  }
  if (old <= max && next <= max) return hunks.flatMap((hunk) => hunk.rows);
  return shown().flatMap((entry) => (entry.kind === "row" ? [entry.row] : []));
}

/** Unchanged lines kept around each change: every one while the whole file is shown. */
export function foldContext(whole: boolean, lines: number): number {
  return whole ? Number.POSITIVE_INFINITY : lines;
}

/** A gap between the hunks the backend sent: its lines are not here until the whole file is. */
export function missingLines(entries: readonly FoldEntry[]): boolean {
  return entries.some((entry) => entry.kind === "gap" && !entry.gap.loaded);
}

/** How long the change a jump lands on stays lit. Must match `.line.flash` in DiffView. */
export const FLASH_MS = 600;

/** The changes Previous and Next go to; one that does not exist turns its button off. */
export interface ChangeSteps {
  prev: number;
  next: number;
}

/** After a jump to change `index`: the ones either side of it. */
export function stepsAround(index: number): ChangeSteps {
  return { prev: index - 1, next: index + 1 };
}

/**
 * Where Previous and Next go from a view scrolled by hand. The view is on a change whose
 * start sits in the anchor rows (`top` to `top + lead`, where a jump puts it), and on the
 * first change while the view is at the very top and it is on screen. Anywhere else it is
 * between two changes: Previous goes back to the one it scrolled past.
 */
export function changeSteps(starts: readonly number[], top: number, bottom: number, lead: number): ChangeSteps {
  let at = -1;
  starts.forEach((start, index) => {
    if (start <= top + lead) at = index;
  });
  if (at === -1) return top === 0 && (starts[0] ?? Infinity) <= bottom ? stepsAround(0) : { prev: -1, next: 0 };
  return (starts[at] ?? 0) >= top ? stepsAround(at) : { prev: at, next: at + 1 };
}

/** The rows a revealed change keeps above it: a third of the view, never fewer than `lead`. */
export function revealLead(viewportHeight: number, rowHeight: number, lead: number): number {
  return Math.max(lead, Math.floor(viewportHeight / rowHeight / 3));
}

/** The scroll offset that puts display row `row` `lead` rows below the top of the view. */
export function revealTop(row: number, lead: number, rowHeight: number): number {
  return Math.max(row - lead, 0) * rowHeight;
}

export function navState(count: number, steps: ChangeSteps): { prev: boolean; next: boolean } {
  const exists = (index: number) => index >= 0 && index < count;
  return { prev: exists(steps.prev), next: exists(steps.next) };
}
