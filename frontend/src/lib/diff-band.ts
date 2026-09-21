/** Geometry of the band between the two columns of a split diff: where it sits, which
    ribbons are worth drawing, and the outline of one. Kept out of the component because
    every one of these is arithmetic that can be checked without a browser. */

/** Width of the strip the ribbons are drawn over. Must match `.band` in DiffView.svelte. */
export const BAND_WIDTH = 28;
/** The line-number gutter of one side. */
export const NUM_WIDTH = 44;

export interface Span {
  fromTop: number;
  fromBottom: number;
  toTop: number;
  toBottom: number;
}

/** `[num][code][band][num][code]` with equal code columns puts the band dead centre.
    Every part is `border-box`, so the widths in the stylesheet are the real ones. */
export function bandLeft(rowsWidth: number): number {
  return Math.max((rowsWidth - BAND_WIDTH) / 2, NUM_WIDTH);
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
