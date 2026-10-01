import type { PaneView } from "./diff-band";
import type { BlockModel } from "./diff-blocks";

/**
 * Synchronized scrolling of the two panes of a split diff.
 *
 * Every block is an anchor pair: it starts at left row `leftRow` and right row `rightRow`
 * and is `leftRows` / `rightRows` tall. In pixels (row height `rh`, `y` measured from the
 * panes' first row, i.e. `scrollTop - top`):
 *
 *   equal zone (leftRows == rightRows):  yR = rightRow*rh + (yL - leftRow*rh)
 *   non-equal block:                     yR = rightRow*rh + (yL - leftRow*rh) * rightRows/leftRows
 *
 * A fold is an equal block of one row per side, so a collapsed zone costs one row and an
 * expansion (a new model) moves every later anchor with it. Above the first row and below
 * the last the mapping is 1:1. A side with no rows in a block (pure insert or delete) has
 * no `y` inside it: the other pane runs through the block while this one holds still at
 * the insertion point, and scrolling this pane past the point continues after the block.
 * The result is rounded once, and clamped to the target's scroll range; a source resting
 * on its bottom puts the target on its own bottom, so panes of different height end flush.
 */

type Side = "left" | "right";

function mapY(model: BlockModel, from: Side, y: number, rh: number): number {
  const rows = (s: Side) => (s === "left" ? model.left.length : model.right.length);
  const to: Side = from === "left" ? "right" : "left";
  const total = rows(from) * rh;
  if (y < 0) return y;
  if (y >= total) return y - total + rows(to) * rh;
  const { blocks } = model;
  const start = (i: number) => (from === "left" ? blocks[i]!.leftRow : blocks[i]!.rightRow) * rh;
  // Last block starting at or above y; a zero-height block at y sorts before the one after it.
  let lo = 0;
  let hi = blocks.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (start(mid) <= y) lo = mid;
    else hi = mid - 1;
  }
  const b = blocks[lo]!;
  const [srcRow, srcRows, dstRow, dstRows] =
    from === "left"
      ? [b.leftRow, b.leftRows, b.rightRow, b.rightRows]
      : [b.rightRow, b.rightRows, b.leftRow, b.leftRows];
  return dstRow * rh + ((y - srcRow * rh) * dstRows) / srcRows;
}

function maxScroll(view: PaneView, rows: number, rh: number): number {
  return Math.max((view.top ?? 0) + rows * rh + (view.bottom ?? 0) - view.viewport, 0);
}

function sync(model: BlockModel, from: Side, source: PaneView, target: PaneView, rh: number): number {
  const sourceRows = from === "left" ? model.left.length : model.right.length;
  const targetRows = from === "left" ? model.right.length : model.left.length;
  const max = maxScroll(target, targetRows, rh);
  if (source.scrollTop > 0 && source.scrollTop >= maxScroll(source, sourceRows, rh)) return max;
  const y = mapY(model, from, source.scrollTop - (source.top ?? 0), rh) + (target.top ?? 0);
  return Math.min(Math.max(Math.round(y), 0), max);
}

/** The right pane's `scrollTop` that lines up with the left pane's. */
export function mapLeftScrollToRight(model: BlockModel, left: PaneView, right: PaneView, rowHeight: number): number {
  return sync(model, "left", left, right, rowHeight);
}

/** The left pane's `scrollTop` that lines up with the right pane's. */
export function mapRightScrollToLeft(model: BlockModel, left: PaneView, right: PaneView, rowHeight: number): number {
  return sync(model, "right", right, left, rowHeight);
}

/** Wheel over the gutter: the same pixel delta for both panes, each kept in its own range. */
export function wheelScroll(
  model: BlockModel,
  left: PaneView,
  right: PaneView,
  delta: number,
  rowHeight: number,
): { left: number; right: number } {
  const clamp = (v: PaneView, rows: number) =>
    Math.min(Math.max(Math.round(v.scrollTop + delta), 0), maxScroll(v, rows, rowHeight));
  return { left: clamp(left, model.left.length), right: clamp(right, model.right.length) };
}
