import type { Block, BlockKind, BlockModel } from "./diff-blocks";

/** Geometry of the band between the two panes of a split diff: where it sits, the connector
    of every block near the viewport, and the outline of one. Kept out of the component
    because every one of these is arithmetic that can be checked without a browser. */

/** Width of the strip the connectors are drawn over. Must match `.band` in DiffView.svelte. */
export const BAND_WIDTH = 42;
/** The line-number gutter of one side. */
export const NUM_WIDTH = 44;
/** The selection gutter before it. Must match `.gutter` in DiffView.svelte. */
export const GUTTER_WIDTH = 14;

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

/** A closed ribbon from pixel edges, left `[a, b]` and right `[x, y]`: down the left edge,
    across on a curve, back up the right edge. */
export function pathOf(a: number, b: number, x: number, y: number): string {
  return `${topCurve(a, x)} L ${BAND_WIDTH} ${y} ${bottomCurve(y, b)} Z`;
}

/** One edge as a path: straight when both ends are level (a curve through equal ends is
    still flattened by the rasterizer and its coverage wobbles along the line). */
function edge(x0: number, y0: number, x1: number, y1: number): string {
  const bend = BAND_WIDTH / 2;
  return y0 === y1
    ? `M ${x0} ${y0} L ${x1} ${y1}`
    : `M ${x0} ${y0} C ${bend} ${y0} ${bend} ${y1} ${x1} ${y1}`;
}

/** Only the two edges, for a 1 px stroke: the vertical sides run along the columns' own
    edges and stroking them would draw a line over the code. The centerline sits half a
    pixel inside the fill, so a level edge covers exactly one pixel row in full color; on
    a whole-number row boundary it straddled two rows at half strength, and two ribbons
    sharing a boundary blended into a pale, uneven line. A side with no height (a wedge)
    keeps its point. */
export function edgesOf(a: number, b: number, x: number, y: number): string {
  const left = Math.min(0.5, (b - a) / 2);
  const right = Math.min(0.5, (y - x) / 2);
  return `${edge(0, a + left, BAND_WIDTH, x + right)} ${edge(BAND_WIDTH, y - right, 0, b - left)}`;
}

/** What one pane shows: its scroll, the height of its viewport, and the header above its
    first row (rows start `top` pixels into the scrolled content). */
export interface PaneView {
  scrollTop: number;
  viewport: number;
  top?: number;
  /** Space after the last row (padding), part of the scrollable height. */
  bottom?: number;
}

/** One block's connector in gutter coordinates (the band's own y axis, whole pixels). */
export interface BlockConnector {
  /** The block it belongs to; for a move, the left half. */
  block: number;
  kind: BlockKind;
  moveId: number | null;
  /** `[top, bottom]` on the left edge and on the right edge. An empty side is a point. */
  left: [number, number];
  right: [number, number];
  /** Filled shape and the two 1 px outlines (see `ribbonEdges` for the pixel lessons):
      paint every fill first, then every outline. */
  path: string;
  edges: string;
  /** Where the block's one action set goes: the exact horizontal center of the gutter, and
      the vertical center of the part of the connector that is on screen. */
  anchor: { x: number; y: number };
}

/** y of a pane row boundary in the gutter: the same row sits at the same y as in the pane. */
function yOf(view: PaneView, row: number, rowHeight: number): number {
  return Math.round((view.top ?? 0) + row * rowHeight - view.scrollTop);
}

function connectorOf(
  block: Block,
  kind: BlockKind,
  moveId: number | null,
  l: [number, number],
  r: [number, number],
  height: number,
  inset: number,
): BlockConnector | null {
  if (Math.min(l[0], r[0]) > height || Math.max(l[1], r[1]) < 0) return null;
  // At the gutter's center the band spans the average of its two edges.
  const top = (l[0] + r[0]) / 2;
  const bottom = (l[1] + r[1]) / 2;
  const lo = Math.max(top, inset);
  const hi = Math.min(bottom, height - inset);
  const mid = lo <= hi ? (lo + hi) / 2 : Math.min(Math.max((top + bottom) / 2, inset), height - inset);
  return {
    block: block.id,
    kind,
    moveId,
    left: l,
    right: r,
    path: pathOf(l[0], l[1], r[0], r[1]),
    edges: edgesOf(l[0], l[1], r[0], r[1]),
    anchor: { x: BAND_WIDTH / 2, y: Math.round(mid) },
  };
}

/**
 * Connectors for the blocks near the viewport, from the panes' scroll positions (SmartGit's
 * ribbons). Plain blocks are found by bisection — their rows are ordered on both sides —
 * so a frame costs O(visible blocks), not O(file). A move joins two far-apart blocks, so
 * moves are all checked; there are few.
 *
 * `inset` keeps the anchor that far from the gutter's top and bottom edge (half a button).
 */
export function blockConnectors(
  model: BlockModel,
  left: PaneView,
  right: PaneView,
  rowHeight: number,
  inset = 0,
): BlockConnector[] {
  const { blocks } = model;
  const height = Math.max(left.viewport, right.viewport);
  const endL = (b: Block) => yOf(left, b.leftRow + b.leftRows, rowHeight);
  const endR = (b: Block) => yOf(right, b.rightRow + b.rightRows, rowHeight);
  const startL = (b: Block) => yOf(left, b.leftRow, rowHeight);
  const startR = (b: Block) => yOf(right, b.rightRow, rowHeight);

  // First block that is not entirely above the viewport on both sides.
  let lo = 0;
  let hi = blocks.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (endL(blocks[mid]!) < 0 && endR(blocks[mid]!) < 0) lo = mid + 1;
    else hi = mid;
  }
  const out: BlockConnector[] = [];
  for (let i = lo; i < blocks.length; i++) {
    const b = blocks[i]!;
    if (startL(b) > height && startR(b) > height) break;
    if (b.kind === "equal" || b.kind === "moved") continue;
    const c = connectorOf(b, b.kind, null, [startL(b), endL(b)], [startR(b), endR(b)], height, inset);
    if (c) out.push(c);
  }

  for (const move of model.moves) {
    const from = move.left === null ? null : blocks[move.left]!;
    const to = move.right === null ? null : blocks[move.right]!;
    const main = (from ?? to)!;
    // An end whose twin is not in the rendered rows meets the point it would have sat at.
    const l: [number, number] = from ? [startL(from), endL(from)] : [startL(to!), startL(to!)];
    const r: [number, number] = to ? [startR(to), endR(to)] : [startR(from!), startR(from!)];
    const c = connectorOf(main, "moved", move.moveId, l, r, height, inset);
    if (c) out.push(c);
  }
  return out.sort((a, b) => Math.min(a.left[0], a.right[0]) - Math.min(b.left[0], b.right[0]));
}
