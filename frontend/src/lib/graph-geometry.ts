/** Single-sourced so the list and the canvas cannot drift apart (doc/12-risks.md, R-03). */

const LANE_WIDTH = { default: 14, min: 8, max: 40 } as const;
let laneWidth: number = LANE_WIDTH.default;

export const GRAPH = {
  rowHeight: 22,
  get laneWidth() {
    return laneWidth;
  },
  leftPad: 10,
  nodeRadius: 3.5,
  mergeRadius: 4.5,
  lineWidth: 1.5,
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

/** Rows before the first commit; list, canvas and hit testing all shift by this. */
export const HEADER_ROWS = 1;

export function toListRow(commitRow: number): number {
  return commitRow + HEADER_ROWS;
}

export function toCommitRow(listRow: number): number | null {
  const row = listRow - HEADER_ROWS;
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
