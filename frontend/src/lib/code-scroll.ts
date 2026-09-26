/**
 * Sideways scrolling of code columns. Rows are virtualized and side-by-side columns move
 * together, so no element is as wide as the code: the view keeps one offset in pixels and
 * moves the text of every column by it (R-470).
 */

/** Where `white-space: pre` puts a tab stop. */
export const TAB_SIZE = 8;

/** Past the widest line, so its last character is not flush with the edge. */
export const TRAILING_COLUMNS = 2;

/** `\ no newline` after the text, with its one-character gap (DiffView `.eof`). */
export const NO_NEWLINE_COLUMNS = 13;

/** Left beside a search hit brought into view, so it does not sit on the edge. */
export const REVEAL_MARGIN_COLUMNS = 4;

const NEEDS_WALK = /[\t̀-ͯᄀ-￿]/;

function charColumns(code: number): number {
  if ((code >= 0x0300 && code <= 0x036f) || (code >= 0x200b && code <= 0x200f)) return 0;
  if (code >= 0xfe00 && code <= 0xfe0f) return 0;
  const wide =
    (code >= 0x1100 && code <= 0x115f) ||
    (code >= 0x2e80 && code <= 0xa4cf && code !== 0x303f) ||
    (code >= 0xac00 && code <= 0xd7a3) ||
    (code >= 0xf900 && code <= 0xfaff) ||
    (code >= 0xfe30 && code <= 0xfe4f) ||
    (code >= 0xff00 && code <= 0xff60) ||
    (code >= 0xffe0 && code <= 0xffe6) ||
    (code >= 0x1f300 && code <= 0x1faff) ||
    (code >= 0x20000 && code <= 0x3fffd);
  return wide ? 2 : 1;
}

/** Columns a line takes in a monospace font: a tab runs to the next stop, a CJK
    character or an emoji takes two, a combining mark none. */
export function textColumns(text: string, tabSize = TAB_SIZE): number {
  if (!NEEDS_WALK.test(text)) return text.length;
  let columns = 0;
  for (const char of text) {
    if (char === "\t") columns += tabSize - (columns % tabSize);
    else columns += charColumns(char.codePointAt(0) ?? 0);
  }
  return columns;
}

export function clampOffset(offset: number, max: number): number {
  return Math.min(Math.max(offset, 0), Math.max(max, 0));
}

/** How far the text can move: the widest line past the width of its column. */
export function maxOffset(columns: number, charWidth: number, view: number): number {
  if (columns <= 0 || charWidth <= 0 || view <= 0) return 0;
  return Math.max(0, Math.ceil(columns * charWidth - view));
}

/**
 * The offset that shows `[from, to)`, in pixels from the start of the line, in a column
 * `view` wide. A range already in view leaves the offset alone; one past an edge is
 * brought in with `margin` to spare, and one wider than the column shows its start.
 */
export function revealOffset(
  offset: number,
  view: number,
  from: number,
  to: number,
  max: number,
  margin = 0,
): number {
  if (from >= offset && to <= offset + view) return clampOffset(offset, max);
  const fits = to - from + 2 * margin <= view;
  const next = !fits || from < offset ? from - margin : to - view + margin;
  return clampOffset(next, max);
}

export interface WheelLike {
  deltaX: number;
  deltaY: number;
  deltaMode: number;
  shiftKey: boolean;
}

/** Sideways pixels a wheel event asks for: a tilt wheel or a trackpad swiped sideways,
    or Shift with the wheel. Zero for a vertical scroll, which stays the browser's. */
export function wheelSideways(event: WheelLike, lineHeight = 18, page = 600): number {
  const scale = event.deltaMode === 1 ? lineHeight : event.deltaMode === 2 ? page : 1;
  if (event.deltaX !== 0 && Math.abs(event.deltaX) >= Math.abs(event.deltaY)) {
    return event.deltaX * scale;
  }
  if (event.shiftKey && event.deltaY !== 0) return event.deltaY * scale;
  return 0;
}
