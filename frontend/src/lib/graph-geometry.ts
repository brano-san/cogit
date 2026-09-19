/**
 * Geometry shared by the virtualized commit list and the canvas that draws under it.
 *
 * Pure so it can be tested without a DOM, and single-sourced so the two layers cannot
 * drift apart — lines sliding relative to their rows is the main risk in this panel
 * (doc/12-risks.md, R-03).
 */

export const GRAPH = {
  rowHeight: 22,
  laneWidth: 14,
  leftPad: 10,
  nodeRadius: 3.5,
  mergeRadius: 4.5,
  lineWidth: 1.5,
  /** The graph must not squeeze out the commit messages. */
  maxGutterFraction: 0.25,
} as const;

export interface VisibleRange {
  start: number;
  end: number;
}

/** Half-open range of rows worth rendering, clamped to the list. */
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

export function laneX(lane: number): number {
  return GRAPH.leftPad + GRAPH.laneWidth * lane;
}

/** Vertical centre of a row in viewport space; the canvas only covers what is visible. */
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

/**
 * Which row and lane a point falls on.
 *
 * Arithmetic rather than `getImageData`: reading pixels back is orders of magnitude
 * slower and would depend on antialiasing.
 */
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

/**
 * Backing-store size for a canvas of the given CSS size.
 *
 * Rounded up: a fractional ratio would otherwise leave the last row a pixel short.
 */
export function canvasPixelSize(cssWidth: number, cssHeight: number, dpr: number) {
  const ratio = dpr > 0 ? dpr : 1;
  return {
    width: Math.ceil(cssWidth * ratio),
    height: Math.ceil(cssHeight * ratio),
  };
}
