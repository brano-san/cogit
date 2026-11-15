import { TAB_SIZE } from "./code-scroll";
import type { BlockModel, PaneRow, UnifiedRow } from "./diff-blocks";

/**
 * Wrapped lines in the diff (F-950). A row of the diff is one line of a side; wrapped, it is
 * as many visual rows as its text takes at the width of its code column, and a list of rows
 * is no longer `index · 18 px`. `RowGeometry` is the one answer to "where is row i": every
 * pixel the view computes from a row (virtualization, connectors, scroll sync, scrollbar
 * marks, jumps) goes through it. Unwrapped it is the old arithmetic (`uniformRows`).
 *
 * Heights come from two places: an estimate for every row, from the advance of each
 * character in the code font (exact for a monospace font, near for the default UI font,
 * R-778), and a measurement of the rows actually drawn, which wins. The CSS wraps at any
 * character (`line-break: anywhere`, `white-space: break-spaces`), so the estimate is the
 * browser's own rule: a row takes characters while they fit.
 */

export interface RowGeometry {
  readonly count: number;
  /** Pixels of every row. */
  readonly total: number;
  /** Top of row `row`; `top(count)` is `total`. */
  top(row: number): number;
  height(row: number): number;
  /** The row at pixel `y`; past the last row the rows go on at the base height, as the
      unwrapped `floor(y / h)` did. */
  rowAt(y: number): number;
}

export function uniformRows(count: number, rowHeight: number): RowGeometry {
  return {
    count,
    total: count * rowHeight,
    top: (row) => row * rowHeight,
    height: () => rowHeight,
    rowAt: (y) => Math.floor(y / rowHeight),
  };
}

/** Rows of `counts[i]` visual rows each (zero allowed: a filler opposite nothing). */
export function wrappedRows(counts: ArrayLike<number>, rowHeight: number): RowGeometry {
  const count = counts.length;
  const tops = new Float64Array(count + 1);
  for (let i = 0; i < count; i++) tops[i + 1] = tops[i]! + counts[i]! * rowHeight;
  const total = tops[count]!;
  return {
    count,
    total,
    top: (row) => (row <= 0 ? row * rowHeight : row >= count ? total + (row - count) * rowHeight : tops[row]!),
    height: (row) => (row >= 0 && row < count ? tops[row + 1]! - tops[row]! : rowHeight),
    rowAt(y) {
      if (y < 0) return Math.floor(y / rowHeight);
      if (y >= total) return count + Math.floor((y - total) / rowHeight);
      // Last row starting at or above y; a row of no height sorts before the one after it.
      let lo = 0;
      let hi = count - 1;
      while (lo < hi) {
        const mid = (lo + hi + 1) >> 1;
        if (tops[mid]! <= y) lo = mid;
        else hi = mid - 1;
      }
      return lo;
    },
  };
}

/** The rows drawn around the viewport, `buffer` rows past each edge. */
export function visibleRows(
  geometry: RowGeometry,
  scrollTop: number,
  viewport: number,
  buffer: number,
): { start: number; end: number } {
  const total = geometry.count;
  if (total <= 0) return { start: 0, end: 0 };
  const start = Math.min(Math.max(geometry.rowAt(scrollTop) - buffer, 0), total);
  const end = Math.min(Math.max(geometry.rowAt(scrollTop + viewport) + 1 + buffer, start), total);
  return { start, end };
}

/** Both panes of a split diff. */
export interface PaneGeometry {
  left: RowGeometry;
  right: RowGeometry;
}

/** A row height (every row alike) or each pane's own geometry. */
export type Heights = number | PaneGeometry;

export function paneGeometry(model: BlockModel, heights: Heights): PaneGeometry {
  return typeof heights === "number"
    ? { left: uniformRows(model.left.length, heights), right: uniformRows(model.right.length, heights) }
    : heights;
}

// --- how many visual rows a line takes ---

/** The advance of one code point in pixels. */
export type Advance = (code: number) => number;

/** Rounding of the fractional advances added up: a row that fits to a hundredth fits. */
const SLACK = 0.01;

/** Visual rows of `text` in a column `width` pixels wide: every character boundary may break,
    a row takes characters while they fit, a tab runs to the next stop from the row's start. */
export function wrapCount(text: string, width: number, advance: Advance, tabSize = TAB_SIZE): number {
  if (width <= 0) return 1;
  const tab = tabSize * advance(32);
  let rows = 1;
  let x = 0;
  for (const char of text) {
    const code = char.codePointAt(0)!;
    let w = code === 9 ? (tab > 0 ? tab - (x % tab) : 0) : advance(code);
    if (x > 0 && w > 0 && x + w > width + SLACK) {
      rows += 1;
      x = 0;
      if (code === 9) w = tab;
    }
    x += w;
  }
  return rows;
}

/** Widths of the code points of one font, measured once each. */
export function advanceTable(measure: (text: string) => number): Advance {
  const bmp = new Float32Array(0x10000).fill(-1);
  const astral = new Map<number, number>();
  return (code) => {
    if (code < 0x10000) {
      const known = bmp[code]!;
      if (known >= 0) return known;
      const w = measure(String.fromCharCode(code));
      bmp[code] = w;
      return w;
    }
    let w = astral.get(code);
    if (w === undefined) astral.set(code, (w = measure(String.fromCodePoint(code))));
    return w;
  };
}

const MAX_TEXTS = 200_000;

/** Estimates for one font: the whole advance of a text (any width), then its wrapped count
    at a width. A few widths are kept: the two panes, and the next one while a divider moves. */
export class WrapEstimate {
  readonly #advance: Advance;
  readonly #tabSize: number;
  readonly #widths = new Map<number, Map<string, number>>();
  readonly #totals = new Map<string, number>();

  constructor(advance: Advance, tabSize = TAB_SIZE) {
    this.#advance = advance;
    this.#tabSize = tabSize;
  }

  rows(text: string, width: number): number {
    if (text.length === 0 || width <= 0) return 1;
    let total = this.#totals.get(text);
    if (total === undefined) {
      // One estimate serves every diff of the page: past a few big files it starts over.
      if (this.#totals.size >= MAX_TEXTS) {
        this.#totals.clear();
        this.#widths.clear();
      }
      total = 0;
      // A tab counted whole: never less than it takes, so "fits" stays true.
      for (const char of text) {
        const code = char.codePointAt(0)!;
        total += code === 9 ? this.#tabSize * this.#advance(32) : this.#advance(code);
      }
      this.#totals.set(text, total);
    }
    if (total <= width + SLACK) return 1;
    let at = this.#widths.get(width);
    if (!at) {
      if (this.#widths.size >= 4) this.#widths.delete(this.#widths.keys().next().value!);
      this.#widths.set(width, (at = new Map()));
    }
    let rows = at.get(text);
    if (rows === undefined) at.set(text, (rows = wrapCount(text, width, this.#advance, this.#tabSize)));
    return rows;
  }
}

const FONTS = new Map<string, WrapEstimate>();

/** The estimate for a CSS font, its advances measured on a canvas; one per font for every
    diff of the page. Without a canvas every character is `charWidth` wide. */
export function fontEstimate(font: string, charWidth: number): WrapEstimate {
  let made = FONTS.get(font);
  if (made) return made;
  const context = document.createElement("canvas").getContext("2d");
  if (context) {
    context.font = font;
    made = new WrapEstimate(advanceTable((text) => context.measureText(text).width));
  } else {
    made = new WrapEstimate(() => charWidth);
  }
  FONTS.set(font, made);
  return made;
}

// --- the visual rows of each list ---

type AnyRow = PaneRow | UnifiedRow;

/** Visual rows of each row of a list: a line by its text (measured if drawn), a fold one row,
    filler none (Aligned: it takes the height of the line opposite). */
export function rowCounts(
  rows: readonly AnyRow[],
  width: number,
  estimate: WrapEstimate,
  measured: ReadonlyMap<number, number>,
): Uint32Array {
  const counts = new Uint32Array(rows.length);
  for (let i = 0; i < rows.length; i++) {
    const row = rows[i]!;
    counts[i] = row.kind === "line" ? (measured.get(i) ?? estimate.rows(row.text, width)) : row.kind === "gap" ? 1 : 0;
  }
  return counts;
}

/** Aligned: a pair is as tall as its taller side, and never less than one row. */
export function pairCounts(left: Uint32Array, right: Uint32Array): Uint32Array {
  const out = new Uint32Array(Math.max(left.length, right.length));
  for (let i = 0; i < out.length; i++) out[i] = Math.max(left[i] ?? 0, right[i] ?? 0, 1);
  return out;
}

/** Where a scroller must go to keep row `anchor` where it was after the rows above it changed
    height: the same offset into that row. */
export function anchoredTop(before: RowGeometry, after: RowGeometry, scrollTop: number): number {
  const anchor = before.rowAt(scrollTop);
  if (anchor < 0 || anchor >= before.count) return scrollTop;
  return after.top(anchor) + Math.min(scrollTop - before.top(anchor), after.height(anchor));
}
