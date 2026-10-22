/** Single-sourced so the list and the canvas cannot drift apart (doc/12-risks.md, R-03). */

export const LANE_WIDTH = { default: 16, min: 12, max: 48 } as const;
let laneWidth: number = LANE_WIDTH.default;

/** Rows of the lists that are not the graph; the graph's own follow its density (#12). */
export const LIST_ROW_HEIGHT = 24;
const ROW_HEIGHT = { min: 16, max: 40 } as const;
let rowHeight: number = LIST_ROW_HEIGHT;

/** CSS pixels; the canvas is scaled by `devicePixelRatio`, so 125% and 150% displays get
    the same shapes with more pixels in them. Lines and rings share one centre, so no
    width has to be nudged half a pixel to meet the other (R-114, R-161). */
export const GRAPH = {
  get rowHeight() {
    return rowHeight;
  },
  get laneWidth() {
    return laneWidth;
  },
  leftPad: 10,
  ringRadius: 3.5,
  ringStroke: 1.5,
  /** The Working Tree's dot: filled, no stroke, smaller than a commit's ring. */
  workingTreeRadius: 2.5,
  /** Lines stop this short of a commit no remote has (R-761). */
  localGap: 1.5,
  lineWidth: 2,
  mainLineWidth: 2.75,
  textGap: 8,
  /** Past this many columns a row is cut off with a fade, and its text starts there. */
  maxColumns: 40,
  arrowLength: 6,
  arrowHead: 3,
} as const;

export interface VisibleRange {
  start: number;
  end: number;
}

export function visibleRange(
  scrollTop: number,
  viewportHeight: number,
  rowHeight: number,
  totalRows: number,
  buffer: number,
): VisibleRange {
  if (totalRows <= 0) return { start: 0, end: 0 };

  const first = Math.floor(scrollTop / rowHeight) - buffer;
  const last = Math.ceil((scrollTop + viewportHeight) / rowHeight) + buffer;

  const start = Math.min(Math.max(first, 0), totalRows);
  const end = Math.min(Math.max(last, start), totalRows);
  return { start, end };
}

/** Blink and WebKit keep layout in 1/64 px of an int32 and saturate near 33.5M px: a taller
    list cannot be scrolled to its end. The scroller stays under this; scroll offsets are
    scaled by `scrollScale` between the element's and the list's own pixels. */
export const MAX_SCROLL_PX = 8_000_000;

/** How many list pixels one scroller pixel stands for: 1 until the list outgrows the cap. */
export function scrollScale(listPx: number, viewportHeight: number): number {
  const real = Math.min(listPx, MAX_SCROLL_PX) - viewportHeight;
  const range = listPx - viewportHeight;
  return real > 0 && range > real ? range / real : 1;
}

/** The list offset an element's `scrollTop` stands for. */
export function toVirtual(real: number, scale: number): number {
  return real * scale;
}

/** The `scrollTop` that shows a list offset. */
export function fromVirtual(virtual: number, scale: number): number {
  return virtual / scale;
}

/** The `scrollTop` that keeps the same list offset when the scale changes under the list: the
    history grows while it is walked, and `k` with it, so the element's own offset would
    stand for a place `k_new / k_old` times further on (GH-09). */
export function rescaledScrollTop(real: number, from: number, to: number): number {
  return fromVirtual(toVirtual(real, from), to);
}

/** How far a wheel event moves the list, in the list's own pixels. The element does the
    scaling itself once the list is taller than the cap — a notch would then jump `k` times
    as many rows — so a scaled list takes the wheel over and moves by what an unscaled one
    would: pixels as given, lines and pages in rows and viewports. */
export function wheelStep(delta: number, mode: number, rowHeight: number, viewportHeight: number): number {
  if (mode === 1) return delta * rowHeight;
  if (mode === 2) return delta * viewportHeight;
  return delta;
}

/** Clamped again here: the canvas must stay drawable whatever the settings file holds. */
export function setLaneWidth(px: number): void {
  laneWidth = Math.min(Math.max(Math.round(px), LANE_WIDTH.min), LANE_WIDTH.max);
}

export function setGraphRowHeight(px: number): void {
  rowHeight = Math.min(Math.max(Math.round(px), ROW_HEIGHT.min), ROW_HEIGHT.max);
}

export function laneX(lane: number): number {
  return GRAPH.leftPad + GRAPH.laneWidth * lane;
}

export function rowY(row: number, scrollTop: number): number {
  return row * GRAPH.rowHeight + GRAPH.rowHeight / 2 - scrollTop;
}

/** Where a commit sits. Lines end here and the dot is drawn here, from one function, so
    they cannot be half a pixel apart again. */
export function nodeCentre(lane: number, row: number, scrollTop: number) {
  return { x: laneX(lane), y: rowY(row, scrollTop) };
}

/** A stash is drawn as a square, on the centre a ring would have. */
export function nodeSquare(lane: number, row: number, scrollTop: number) {
  const { x, y } = nodeCentre(lane, row, scrollTop);
  const size = GRAPH.ringRadius * 2;
  return { x: x - size / 2, y: y - size / 2, size };
}

/** Every second row a shade lighter (#21); the Working Tree row is the first, unstriped.
    The same in Files, Repositories, Branches and Worktrees (#41). `listRow` is the row's
    place in the list as drawn, never in the DOM: a virtual list mounts from wherever the
    view starts. `stripes` is the Preferences switch that turns the banding off. */
export function striped(listRow: number, stripes = true): boolean {
  return stripes && listRow % 2 === 1;
}

/** The row each item of a list starts on, when an item draws `sizes[i]` rows: a repository
    and the submodules open under it, or none for one the list leaves out. */
export function rowStarts(sizes: readonly number[]): number[] {
  let at = 0;
  return sizes.map((size) => {
    const start = at;
    at += size;
    return start;
  });
}

/** What is behind a node, bottom up, so its fill hides the lines exactly as the row does:
    the panel, the stripe, then hover and selection, which cover the stripe. `selectedRows`
    are every row drawn selected: the selection and the other end of a comparison. `tint`:
    the row's own background, which takes the stripe's place (a bisect mark, F-566). */
export function nodeFill(
  listRow: number,
  selectedRows: readonly number[],
  hoverRow: number | null,
  stripes = true,
  tint: string | null = null,
): string[] {
  const layers = ["--surface-panel"];
  if (tint) layers.push(tint);
  else if (striped(listRow, stripes)) layers.push("--row-stripe");
  if (listRow === hoverRow) layers.push("--state-hover");
  if (selectedRows.includes(listRow)) layers.push("--state-selected");
  return layers;
}

export function textX(width: number): number {
  const columns = Math.min(Math.max(width, 1), GRAPH.maxColumns);
  return laneX(columns - 1) + GRAPH.laneWidth / 2 + GRAPH.textGap;
}

export interface Curve {
  x1: number;
  y1: number;
  cx1: number;
  cy1: number;
  cx2: number;
  cy2: number;
  x2: number;
  y2: number;
}

/** Where along the chord the curve leaves the node, and how far up the column it rounds
    into it: a slant out of the ring, upright at the far end (SmartGit's `<`). */
const TURN = 0.5;

/** A line changing column at a node, in one shape for a branch leaving its parent upwards
    and a merge going down to its second parent: from the node's centre to the centre of
    the next row in column `toX`. The two are mirror images, as are left and right. */
export function transition(node: { x: number; y: number }, toX: number, direction: "up" | "down"): Curve {
  const dy = direction === "down" ? GRAPH.rowHeight : -GRAPH.rowHeight;
  const y2 = node.y + dy;
  return {
    x1: node.x,
    y1: node.y,
    cx1: node.x + (toX - node.x) * TURN,
    cy1: node.y + dy * TURN,
    cx2: toX,
    cy2: y2 - dy * TURN,
    x2: toX,
    y2,
  };
}

type Drawn = { from: number; to: number; span: "top" | "bottom" | "through"; arrow?: boolean };

const turnsDown = (segments: readonly Drawn[] | undefined, lane: number) =>
  segments?.find((s) => !s.arrow && s.span === "bottom" && s.to === lane && s.from !== lane);
const turnsUp = (segments: readonly Drawn[] | undefined, lane: number) =>
  segments?.find((s) => !s.arrow && s.span === "top" && s.from === lane && s.to !== lane);

/** A segment of row `listRow` (counting the header rows), or `null` when a turn drawn by a
    neighbouring row covers it. A turn reaches half a row into the next row, so that row's
    straight half in the same column is left out; `above` and `below` are the segments of
    the rows next to it, absent when not on hand (the header row, the ends of the list). */
export function segmentCurve(
  segment: Drawn,
  listRow: number,
  scrollTop: number,
  { above, below }: { above?: readonly Drawn[]; below?: readonly Drawn[] } = {},
): Curve | null {
  const top = listRow * GRAPH.rowHeight - scrollTop;
  const centre = top + GRAPH.rowHeight / 2;
  const bottom = top + GRAPH.rowHeight;
  if (segment.from !== segment.to && segment.span !== "through") {
    if (segment.span === "bottom") {
      const next = turnsUp(below, segment.to);
      return transition({ x: laneX(segment.from), y: centre }, laneX(next ? next.to : segment.to), "down");
    }
    // Two turns in a row, out of one node and into the next: the row above draws it whole.
    if (turnsDown(above, segment.from)) return null;
    return transition({ x: laneX(segment.to), y: centre }, laneX(segment.from), "up");
  }
  let y1 = segment.span === "bottom" ? centre : top;
  let y2 = segment.span === "top" ? centre : bottom;
  if (y1 === top && turnsDown(above, segment.from)) y1 = centre;
  if (y2 === bottom && turnsUp(below, segment.to)) y2 = centre;
  if (y1 >= y2) return null;
  const x1 = laneX(segment.from);
  const x2 = laneX(segment.to);
  const middle = (y1 + y2) / 2;
  return { x1, y1, cx1: x1, cy1: middle, cx2: x2, cy2: middle, x2, y2 };
}

/** Where HEAD's line in `lane` starts across the Working Tree row at `headerRow`: its top,
    or its centre where the commit above turns into that column, its turn ending there. */
export function throughWorkingTree(
  headerRow: number,
  scrollTop: number,
  above: readonly Drawn[] | undefined,
  lane: number,
): number {
  const top = headerRow * GRAPH.rowHeight - scrollTop;
  return turnsDown(above, lane) ? top + GRAPH.rowHeight / 2 : top;
}

/** The curve as an SVG path, for `Path2D` and for snapshots. */
export function svgPath(curve: Curve): string {
  const n = (value: number) => String(Math.round(value * 100) / 100);
  const { x1, y1, cx1, cy1, cx2, cy2, x2, y2 } = curve;
  if (x1 === x2 && cx1 === x1 && cx2 === x2) return `M${n(x1)} ${n(y1)}V${n(y2)}`;
  return `M${n(x1)} ${n(y1)}C${n(cx1)} ${n(cy1)} ${n(cx2)} ${n(cy2)} ${n(x2)} ${n(y2)}`;
}

/** The line to a commit the list does not show, or to the far end of a cut link: a stub
    under the ring, pointing on; it leans right when it is not the first parent, whose line
    goes straight down. A `top` stub is the parent's end of a cut link, coming down into the
    ring from the upper right (R-330). */
export function arrowStub(
  segment: { from: number; to: number; span?: "top" | "bottom" | "through" },
  listRow: number,
  scrollTop: number,
) {
  const { x, y } = nodeCentre(segment.from, listRow, scrollTop);
  const lean = segment.to > segment.from ? Math.SQRT1_2 : 0;
  const down = lean > 0 ? Math.SQRT1_2 : 1;
  const up = segment.span === "top";
  const start = GRAPH.ringRadius + GRAPH.ringStroke;
  const length = Math.min(GRAPH.arrowLength, GRAPH.rowHeight / 2 / down - start);
  const out = { x: lean, y: up ? -down : down };
  const near = { x: x + out.x * start, y: y + out.y * start };
  const far = { x: near.x + out.x * length, y: near.y + out.y * length };
  const [x1, y1, x2, y2] = up ? [far.x, far.y, near.x, near.y] : [near.x, near.y, far.x, far.y];
  // The way the arrow travels: away from the ring below it, into the ring above it.
  const dx = up ? -out.x : out.x;
  const dy = up ? -out.y : out.y;
  const h = GRAPH.arrowHead;
  return {
    x1,
    y1,
    x2,
    y2,
    left: { x: x2 - (dx + dy) * h, y: y2 - (dy - dx) * h },
    right: { x: x2 - (dx - dy) * h, y: y2 - (dy + dx) * h },
  };
}

export interface GraphHit {
  row: number;
  lane: number;
}

/** Arithmetic, not `getImageData`: pixel readback is far slower and antialiasing-dependent. */
export function hitTest(
  x: number,
  y: number,
  scrollTop: number,
  totalRows: number,
): GraphHit | null {
  const absoluteY = y + scrollTop;
  if (absoluteY < 0) return null;

  const row = Math.floor(absoluteY / GRAPH.rowHeight);
  if (row < 0 || row >= totalRows) return null;

  const lane = Math.round((x - GRAPH.leftPad) / GRAPH.laneWidth);
  if (lane < 0) return null;

  return { row, lane };
}

export function canvasPixelSize(cssWidth: number, cssHeight: number, dpr: number) {
  const ratio = dpr > 0 ? dpr : 1;
  return {
    width: Math.ceil(cssWidth * ratio),
    height: Math.ceil(cssHeight * ratio),
  };
}

/** The Working Tree row. A rebase in progress adds its own rows on top of it, so the
    offset is a parameter everywhere and this is only the floor. */
export const HEADER_ROWS = 1;

/** `at`: the commit row the header rows sit above — HEAD's when it is not the first, else 0. */
export function toListRow(commitRow: number, headerRows: number = HEADER_ROWS, at = 0): number {
  return commitRow < at ? commitRow : commitRow + headerRows;
}

export function toCommitRow(listRow: number, headerRows: number = HEADER_ROWS, at = 0): number | null {
  if (listRow < at) return listRow;
  const row = listRow - headerRows;
  return row >= at ? row : null;
}

/** The Working Tree row's lane above HEAD: HEAD's own when no line comes into it from above,
    else the first lane right of it no line crosses there. Like a merge's second parent, the
    Working Tree reserves nothing: the line from above keeps HEAD's lane (R-783). */
export function workingTreeLane(headLane: number, segments: readonly { from: number; span: string }[]): number {
  const taken = new Set(segments.filter((segment) => segment.span !== "bottom").map((segment) => segment.from));
  if (!taken.has(headLane)) return headLane;
  let lane = headLane + 1;
  while (taken.has(lane)) lane++;
  return lane;
}

/** HEAD's ring, where the dashed line from the Working Tree row ends (07 §4). It is not
    always the first commit: HEAD comes under whatever of its descendants is shown (R-164). */
export function headNode(
  headRow: number | null,
  laneAt: (commitRow: number) => number | undefined,
  headerRows: number = HEADER_ROWS,
  at = 0,
): { lane: number; listRow: number } | null {
  if (headRow === null) return null;
  const lane = laneAt(headRow);
  return lane === undefined ? null : { lane, listRow: toListRow(headRow, headerRows, at) };
}

/** What a click on a list row selects: a commit, the working tree (`null`, the first row),
    or nothing — a rebase row, or a commit whose block has not arrived yet. */
export function clickedCommit(
  listRow: number,
  headerRows: number,
  oidAt: (commitRow: number) => string | undefined,
  at = 0,
): string | null | undefined {
  const row = toCommitRow(listRow, headerRows, at);
  if (row === null) return listRow === at ? null : undefined;
  return oidAt(row);
}

export function nextRow(
  current: number | null,
  key: string,
  total: number,
  pageRows: number,
): number | null {
  if (total <= 0) return null;
  const at = current ?? -1;
  const clamp = (row: number) => Math.min(Math.max(row, 0), total - 1);

  switch (key) {
    case "ArrowDown":
      return current === null ? 0 : clamp(at + 1);
    case "ArrowUp":
      return current === null ? 0 : clamp(at - 1);
    case "PageDown":
      return clamp(Math.max(at, 0) + pageRows);
    case "PageUp":
      return clamp(Math.max(at, 0) - pageRows);
    case "Home":
      return 0;
    case "End":
      return total - 1;
    default:
      return null;
  }
}

/** Where a key press lands, counted from the selected commit. Its block may be evicted or
    not inherited by a reload, so Rust is asked for its row (R-193); only a commit this
    graph lacks starts from the top. */
export async function keyTarget(
  rows: { loadedIndexOf(oid: string | null): number | null; indexOf(oid: string): Promise<number | null> },
  selected: string | null,
  key: string,
  total: number,
  pageRows: number,
): Promise<number | null> {
  const at = rows.loadedIndexOf(selected) ?? (selected === null ? null : await rows.indexOf(selected));
  return nextRow(at, key, total, pageRows);
}

export function scrollRowIntoView(
  row: number,
  scrollTop: number,
  viewportHeight: number,
  rowHeight: number,
): number | null {
  const top = row * rowHeight;
  const bottom = top + rowHeight;
  if (top < scrollTop) return Math.max(top, 0);
  if (bottom > scrollTop + viewportHeight) return Math.max(bottom - viewportHeight, 0);
  return null;
}

/** Centres, unlike `scrollRowIntoView`: a row reached from another panel needs context. */
export function centreRow(
  row: number,
  viewportHeight: number,
  rowHeight: number,
  totalRows: number,
): number {
  const wanted = row * rowHeight + rowHeight / 2 - viewportHeight / 2;
  const furthest = Math.max(totalRows * rowHeight - viewportHeight, 0);
  return Math.min(Math.max(wanted, 0), furthest);
}

/** The list row under a pointer, from the scroller's live offset (`y` is measured from the
    top of its viewport). Not from the element under the pointer: rows are moved by a
    transform that trails a fast scroll by a frame or more, the scroll offset does not. */
export function listRowAt(y: number, scrollTop: number, rowHeight: number, listRows: number): number | null {
  if (y < 0 || rowHeight <= 0) return null;
  const row = Math.floor((y + scrollTop) / rowHeight);
  return row < listRows ? row : null;
}
