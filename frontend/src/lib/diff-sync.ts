import type { PaneView } from "./diff-band";
import type { BlockModel } from "./diff-blocks";
import { paneGeometry, type Heights, type PaneGeometry, type RowGeometry } from "./diff-wrap";

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
 * With wrapped lines (F-950) `row*rh` is the row's top in its pane's `RowGeometry` and a
 * block's length is its pixel height, so a block maps onto its twin however its lines wrap.
 *
 * A fold is an equal block of one row per side, so a collapsed zone costs one row and an
 * expansion (a new model) moves every later anchor with it. Above the first row and below
 * the last the mapping is 1:1. A side with no rows in a block (pure insert or delete) has
 * no `y` inside it: the other pane runs through the block while this one holds still at
 * the insertion point, and scrolling this pane past the point continues after the block.
 * The result is rounded once, and clamped to the target's scroll range; a source resting
 * on its bottom puts the target on its own bottom, so panes of different height end flush;
 * over the source's last screenful the target is eased there, so the end is not a jump (R-628).
 */

type Side = "left" | "right";

function mapY(model: BlockModel, rows: PaneGeometry, from: Side, y: number): number {
  const src = rows[from];
  const to: Side = from === "left" ? "right" : "left";
  const dst = rows[to];
  if (y < 0) return y;
  if (y >= src.total) return y - src.total + dst.total;
  const { blocks } = model;
  const start = (i: number) => src.top(from === "left" ? blocks[i]!.leftRow : blocks[i]!.rightRow);
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
  const srcTop = src.top(srcRow);
  const dstTop = dst.top(dstRow);
  return dstTop + ((y - srcTop) * (dst.top(dstRow + dstRows) - dstTop)) / (src.top(srcRow + srcRows) - srcTop);
}

function maxScroll(view: PaneView, rows: RowGeometry): number {
  return Math.max((view.top ?? 0) + rows.total + (view.bottom ?? 0) - view.viewport, 0);
}

function sync(model: BlockModel, from: Side, source: PaneView, target: PaneView, heights: Heights): number {
  const rows = paneGeometry(model, heights);
  const max = maxScroll(target, rows[from === "left" ? "right" : "left"]);
  const sourceMax = maxScroll(source, rows[from]);
  if (source.scrollTop > 0 && source.scrollTop >= sourceMax) return max;
  const at = (top: number) => mapY(model, rows, from, top - (source.top ?? 0)) + (target.top ?? 0);
  let y = at(source.scrollTop);
  // Over the last screenful of the source the target is eased by the gap between where the
  // blocks would put it at the very end and its own bottom, so reaching the end does not jump
  // it by the difference of the two lengths (and equal panes, with no gap, are left alone).
  const zone = Math.min(source.viewport, sourceMax);
  const into = source.scrollTop - (sourceMax - zone);
  if (zone > 0 && into > 0) y += (max - at(sourceMax)) * (into / zone);
  return Math.min(Math.max(Math.round(y), 0), max);
}

/** The right pane's `scrollTop` that lines up with the left pane's. */
export function mapLeftScrollToRight(model: BlockModel, left: PaneView, right: PaneView, heights: Heights): number {
  return sync(model, "left", left, right, heights);
}

/** The left pane's `scrollTop` that lines up with the right pane's. */
export function mapRightScrollToLeft(model: BlockModel, left: PaneView, right: PaneView, heights: Heights): number {
  return sync(model, "right", right, left, heights);
}

/** Wheel over the gutter: the same pixel delta for both panes, each kept in its own range. */
export function wheelScroll(
  model: BlockModel,
  left: PaneView,
  right: PaneView,
  delta: number,
  heights: Heights,
): { left: number; right: number } {
  const rows = paneGeometry(model, heights);
  const clamp = (v: PaneView, of: RowGeometry) =>
    Math.min(Math.max(Math.round(v.scrollTop + delta), 0), maxScroll(v, of));
  return { left: clamp(left, rows.left), right: clamp(right, rows.right) };
}
