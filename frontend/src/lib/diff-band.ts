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
  /** Where the block's buttons go (R-628): » in the left part of the gutter, × in the
      right part (`ACTION_LEFT_X`, `ACTION_RIGHT_X`), each at the vertical center of the part of
      the band that is on screen *at its own x*, so it sits on its connector however the band
      slants. A `changed` block has one » for the whole block, in the middle (`center`,
      `ACTION_CENTER_X`; R-629). `null`: the band is not on screen there, no button. */
  anchor: { left: number | null; right: number | null; center: number | null };
}

/** Horizontal centers of the two buttons: the middles of the gutter's halves. */
export const ACTION_LEFT_X = BAND_WIDTH / 4;
export const ACTION_RIGHT_X = (BAND_WIDTH * 3) / 4;
/** The lone » of a changed block: the middle of the gutter. */
export const ACTION_CENTER_X = BAND_WIDTH / 2;

/** How far along the S-curve of the band (0 at the left column, 1 at the right) the point at
    `x` is. The curve's controls sit at the horizontal midpoint, so
    x(t) = 1.5·W·t(1−t) + W·t³ and y = from + (to − from)·(3t² − 2t³). */
export function curveFraction(x: number): number {
  const w = BAND_WIDTH;
  let lo = 0;
  let hi = 1;
  for (let i = 0; i < 30; i++) {
    const t = (lo + hi) / 2;
    if (1.5 * w * t * (1 - t) + w * t ** 3 < x) lo = t;
    else hi = t;
  }
  const t = (lo + hi) / 2;
  return 3 * t * t - 2 * t * t * t;
}

const LEFT_FRACTION = curveFraction(ACTION_LEFT_X);
const RIGHT_FRACTION = curveFraction(ACTION_RIGHT_X);
const CENTER_FRACTION = curveFraction(ACTION_CENTER_X);

/** The center of what is on screen of a band that spans `[top, bottom]` there, kept half a
    button (`inset`) from the gutter's edges; `null` when none of it is on screen. */
export function visibleCenter(top: number, bottom: number, height: number, inset: number): number | null {
  if (bottom < 0 || top > height) return null;
  const lo = Math.max(top, inset);
  const hi = Math.min(bottom, height - inset);
  const mid = lo <= hi ? (lo + hi) / 2 : Math.min(Math.max((top + bottom) / 2, inset), height - inset);
  return Math.round(mid);
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
  // At `x` the band spans the edges blended by the curve there.
  const at = (f: number) => {
    return visibleCenter(l[0] + (r[0] - l[0]) * f, l[1] + (r[1] - l[1]) * f, height, inset);
  };
  return {
    block: block.id,
    kind,
    moveId,
    left: l,
    right: r,
    path: pathOf(l[0], l[1], r[0], r[1]),
    edges: edgesOf(l[0], l[1], r[0], r[1]),
    anchor: { left: at(LEFT_FRACTION), right: at(RIGHT_FRACTION), center: at(CENTER_FRACTION) },
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
