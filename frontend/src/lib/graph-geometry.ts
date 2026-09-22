/** Single-sourced so the list and the canvas cannot drift apart (doc/12-risks.md, R-03). */

const LANE_WIDTH = { default: 18, min: 12, max: 48 } as const;
let laneWidth: number = LANE_WIDTH.default;

/** Sizes are CSS pixels; the canvas is scaled by `devicePixelRatio` before drawing, so
    125% and 150% displays get the same shapes with more pixels in them.

    `lineWidth` is even on purpose. An odd stroke has to be nudged half a pixel to land
    on a whole one, the dots never were, and the line ran half a pixel off the dot it was
    supposed to pass through (doc/12-risks.md, R-114). */
export const GRAPH = {
  rowHeight: 24,
  get laneWidth() {
    return laneWidth;
  },
  leftPad: 12,
  nodeRadius: 4,
  mergeRadius: 5,
  lineWidth: 2,
  maxGutterFraction: 0.25,
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

export function gutterWidth(maxLane: number, panelWidth: number): number {
  const natural = GRAPH.leftPad + GRAPH.laneWidth * (maxLane + 1);
  return Math.min(natural, panelWidth * GRAPH.maxGutterFraction);
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

export interface RowIndexed {
  fromRow: number;
}

/** Bucketed by upper row: rescanning every edge each frame misses the frame budget. */
export function indexByRow<T extends RowIndexed>(edges: readonly T[]): Map<number, T[]> {
  const index = new Map<number, T[]>();
  for (const edge of edges) {
    const bucket = index.get(edge.fromRow);
    if (bucket) bucket.push(edge);
    else index.set(edge.fromRow, [edge]);
  }
  return index;
}

/** Edges that cross the rows on screen, plus the band just above so lines enter correctly. */
export function edgeBand<T extends RowIndexed>(
  index: Map<number, T[]>,
  firstRow: number,
  lastRow: number,
): T[] {
  const band: T[] = [];
  for (let row = firstRow - 1; row <= lastRow; row++) {
    const bucket = index.get(row);
    if (bucket) band.push(...bucket);
  }
  return band;
}

/** Keyboard navigation over the list; `null` means the key was not ours. */
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

/** The new scroll offset, or `null` when the row already fits on screen. */
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
