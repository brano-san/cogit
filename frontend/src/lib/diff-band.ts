/** Geometry of the band between the two columns of a split diff: where it sits, which
    ribbons are worth drawing, and the outline of one. Kept out of the component because
    every one of these is arithmetic that can be checked without a browser. */

/** Width of the strip the ribbons are drawn over. Must match `.band` in DiffView.svelte. */
export const BAND_WIDTH = 42;
/** The line-number gutter of one side. */
export const NUM_WIDTH = 44;
/** The selection gutter before it. Must match `.gutter` in DiffView.svelte. */
export const GUTTER_WIDTH = 14;

export interface Span {
  fromTop: number;
  fromBottom: number;
  toTop: number;
  toBottom: number;
}

/** Neither code column goes below a fifth of the code width: the other side still shows. */
export const SPLIT_MIN = 0.2;
export const SPLIT_MAX = 0.8;
/** What the arrow keys move the divider by; Home puts it back in the middle. */
export const SPLIT_STEP = 0.02;
export const SPLIT_EVEN = 0.5;

function codeWidth(rowsWidth: number, sideWidth: number): number {
  return Math.max(rowsWidth - 2 * sideWidth - BAND_WIDTH, 0);
}

/** `[gutter][num][sign][code][band][num][sign][code][gutter]`: the fixed columns of a side
    are `sideWidth`, and the left code column is `share` of what both code columns get.
    Every part is `border-box`, so the widths in the stylesheet are the real ones. */
export function bandLeft(rowsWidth: number, share: number, sideWidth: number): number {
  return Math.max(sideWidth + codeWidth(rowsWidth, sideWidth) * share, NUM_WIDTH);
}

export function clampShare(share: number): number {
  return Math.min(Math.max(share, SPLIT_MIN), SPLIT_MAX);
}

/** The share of the left code column once the divider moved `delta` pixels (R-535). */
export function draggedShare(share: number, delta: number, rowsWidth: number, sideWidth: number): number {
  const code = codeWidth(rowsWidth, sideWidth);
  return code === 0 ? share : clampShare(share + delta / code);
}

/** Only what is near the viewport is drawn; the band is as tall as the whole file. */
export function ribbonsNear<T extends Span>(all: readonly T[], from: number, to: number): T[] {
  return all.filter(
    (c) => Math.max(c.fromBottom, c.toBottom) >= from && Math.min(c.fromTop, c.toTop) <= to,
  );
}

/** A closed ribbon: down the left edge, across on a curve, back up the right edge. */
function corners(c: Span, rowHeight: number) {
  return {
    a: c.fromTop * rowHeight,
    b: (c.fromBottom + 1) * rowHeight,
    x: c.toTop * rowHeight,
    y: (c.toBottom + 1) * rowHeight,
  };
}

/** Top edge (left to right) as a cubic whose controls sit at the horizontal midpoint, so
    both ends leave horizontally and meet the columns without a kink. */
function topCurve(a: number, x: number): string {
  const bend = BAND_WIDTH / 2;
  return `M 0 ${a} C ${bend} ${a} ${bend} ${x} ${BAND_WIDTH} ${x}`;
}

/** Bottom edge, right to left. A zero-height side (pure insert/delete) collapses its end
    to a point, so the ribbon becomes a wedge. */
function bottomCurve(y: number, b: number): string {
  const bend = BAND_WIDTH / 2;
  return `C ${bend} ${y} ${bend} ${b} 0 ${b}`;
}

/** A closed ribbon: down the left edge, across on a curve, back up the right edge. */
export function ribbonPath(c: Span, rowHeight: number): string {
  const { a, b, x, y } = corners(c, rowHeight);
  return `${topCurve(a, x)} L ${BAND_WIDTH} ${y} ${bottomCurve(y, b)} Z`;
}

/** Only the two curves, for a stroke: the vertical sides run along the columns' own edges
    and stroking them would draw a line over the code. */
export function ribbonEdges(c: Span, rowHeight: number): string {
  const { a, b, x, y } = corners(c, rowHeight);
  return `${topCurve(a, x)} M ${BAND_WIDTH} ${y} ${bottomCurve(y, b)}`;
}
