/** Geometry of the band between the two columns of a split diff: where it sits, which
    ribbons are worth drawing, and the outline of one. Kept out of the component because
    every one of these is arithmetic that can be checked without a browser. */

/** Width of the strip the ribbons are drawn over. Must match `.band` in DiffView.svelte. */
export const BAND_WIDTH = 28;
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

/** `[gutter][num][sign][code][band][gutter][num][sign][code]`: the fixed columns of a side
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
export function ribbonPath(c: Span, rowHeight: number): string {
  const w = BAND_WIDTH;
  const bend = w / 2;
  const a = c.fromTop * rowHeight;
  const b = (c.fromBottom + 1) * rowHeight;
  const x = c.toTop * rowHeight;
  const y = (c.toBottom + 1) * rowHeight;
  return `M 0 ${a} C ${bend} ${a} ${bend} ${x} ${w} ${x} L ${w} ${y} C ${bend} ${y} ${bend} ${b} 0 ${b} Z`;
}
