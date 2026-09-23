/** Single-sourced so the list and the canvas cannot drift apart (doc/12-risks.md, R-03). */

const LANE_WIDTH = { default: 16, min: 12, max: 48 } as const;
let laneWidth: number = LANE_WIDTH.default;

/** CSS pixels; the canvas is scaled by `devicePixelRatio`, so 125% and 150% displays get
    the same shapes with more pixels in them. Lines and rings share one centre, so no
    width has to be nudged half a pixel to meet the other (R-114, R-161). */
export const GRAPH = {
  rowHeight: 24,
  get laneWidth() {
    return laneWidth;
  },
  leftPad: 10,
  ringRadius: 3.5,
  ringStroke: 1.5,
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

/** Clamped again here: the canvas must stay drawable whatever the settings file holds. */
export function setLaneWidth(px: number): void {
  laneWidth = Math.min(Math.max(Math.round(px), LANE_WIDTH.min), LANE_WIDTH.max);
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

/** A stash is drawn as a square, as SmartGit does, on the centre a ring would have. */
export function nodeSquare(lane: number, row: number, scrollTop: number) {
  const { x, y } = nodeCentre(lane, row, scrollTop);
  const size = GRAPH.ringRadius * 2;
  return { x: x - size / 2, y: y - size / 2, size };
}

/** Every second row a shade lighter (#21); the Working Tree row is the first, unstriped. */
export function striped(listRow: number): boolean {
  return listRow % 2 === 1;
}

/** What is behind a node, bottom up, so its fill hides the lines exactly as the row does:
    the panel, the stripe, then hover and selection, which cover the stripe. */
export function nodeFill(listRow: number, selectedRow: number | null, hoverRow: number | null): string[] {
  const layers = ["--surface-panel"];
  if (striped(listRow)) layers.push("--row-stripe");
  if (listRow === hoverRow) layers.push("--state-hover");
  if (listRow === selectedRow) layers.push("--state-selected");
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

/** A cubic with both tangents vertical: a straight line when the column stays, an S
    inside the one row when it changes. `listRow` counts the header rows. */
export function segmentCurve(
  segment: { from: number; to: number; span: "top" | "bottom" | "through" },
  listRow: number,
  scrollTop: number,
): Curve {
  const top = listRow * GRAPH.rowHeight - scrollTop;
  const centre = top + GRAPH.rowHeight / 2;
  const bottom = top + GRAPH.rowHeight;
  const y1 = segment.span === "bottom" ? centre : top;
  const y2 = segment.span === "top" ? centre : bottom;
  const x1 = laneX(segment.from);
  const x2 = laneX(segment.to);
  const middle = (y1 + y2) / 2;
  return { x1, y1, cx1: x1, cy1: middle, cx2: x2, cy2: middle, x2, y2 };
}

/** The line to a commit the list does not show: a stub under the ring, pointing on; it
    leans right when it is not the first parent, whose line goes straight down. */
export function arrowStub(
  segment: { from: number; to: number },
  listRow: number,
  scrollTop: number,
) {
  const { x, y } = nodeCentre(segment.from, listRow, scrollTop);
  const dx = segment.to > segment.from ? Math.SQRT1_2 : 0;
  const dy = dx > 0 ? Math.SQRT1_2 : 1;
  const start = GRAPH.ringRadius + GRAPH.ringStroke;
  const length = Math.min(GRAPH.arrowLength, GRAPH.rowHeight / 2 / dy - start);
  const x1 = x + dx * start;
  const y1 = y + dy * start;
  const x2 = x1 + dx * length;
  const y2 = y1 + dy * length;
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

export function toListRow(commitRow: number, headerRows: number = HEADER_ROWS): number {
  return commitRow + headerRows;
}

export function toCommitRow(listRow: number, headerRows: number = HEADER_ROWS): number | null {
  const row = listRow - headerRows;
  return row >= 0 ? row : null;
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
