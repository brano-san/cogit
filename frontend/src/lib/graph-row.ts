import type { GraphColumn, GraphDensity, GraphTimeFormat } from "$lib/settings";
import { textX } from "$lib/graph-geometry";

/** A commit row of the graph, driven by the graph settings (#12). Right columns are pinned
    to the right edge; the middle gives way, the branch labels first, then the subject; past
    that the graph area is cut at its edge rather than its lanes squeezed (R-331). */

export type { GraphColumn, GraphDensity, GraphTimeFormat };
import { graphTime } from "$lib/graph-columns";
export { graphTime };

/** How the list looks until Preferences say otherwise: as it always has. */
export const GRAPH_COLUMNS: readonly GraphColumn[] = ["author", "avatar", "time", "hash"];
export const GRAPH_TIME_FORMAT: GraphTimeFormat = "dateTime";
export const GRAPH_DENSITY: GraphDensity = "normal";
export const GRAPH_STRIPES = true;
/** A little over a screen of the Graph panel at 1080p (R-330). */
export const LONG_LINK_ROWS = 40;

export const DENSITY_ROW_HEIGHT: Record<GraphDensity, number> = {
  compact: 20,
  normal: 24,
  comfortable: 28,
};

/** Pixels. Fixed, so the columns of every row line up and the room they take is known;
    the author is its widest, a shorter name leaves the rest to the subject. */
export const COLUMN_WIDTH = { author: 140, avatar: 16, hash: 48, overlap: 74 } as const;

/** Every shape a format writes, for measuring the widest: now, minutes and hours ago, each
    day of the last week, a month ago, last year with two-digit day, month and hour. */
export function timeSamples(format: GraphTimeFormat, now: number): string[] {
  const day = 86_400;
  const spans = [0, 59, 3_540, 18_000, day, 2 * day, 3 * day, 4 * day, 5 * day, 6 * day, 7 * day, 20 * day, 45 * day];
  const today = new Date(now * 1000);
  const old = Date.UTC(today.getUTCFullYear() - 1, 11, 28, 23, 58) / 1000;
  const years = Date.UTC(today.getUTCFullYear() - 12, 11, 28, 23, 58) / 1000;
  return [...spans.map((ago) => now - ago), old, years].map((when) => graphTime(when, 0, now, format));
}

/** The widest of `timeSamples` as `measure` sees it, whole pixels. */
export function measuredTimeWidth(format: GraphTimeFormat, now: number, measure: (text: string) => number): number {
  return Math.ceil(Math.max(...timeSamples(format, now).map(measure)));
}

/** Where nothing can measure text (a test, a headless run): the widths measured once by hand. */
export function timeWidth(format: GraphTimeFormat): number {
  switch (format) {
    case "date":
      return 74;
    case "relative":
      return 84;
    case "dateTime":
      return 110;
  }
}

export interface RightColumns {
  columns: readonly GraphColumn[];
  /** Avatars off draw nothing, so the column takes no room. */
  avatars: boolean;
  time: GraphTimeFormat;
  overlap: boolean;
  /** The row's gap and right padding, from the design tokens. */
  gap: number;
  padding: number;
}

/** The most the right columns can take, gaps included; the author may take less. */
export function rightColumnsWidth(spec: RightColumns): number {
  const widths: number[] = [];
  for (const column of spec.columns) {
    if (column === "avatar" && !spec.avatars) continue;
    widths.push(column === "time" ? timeWidth(spec.time) : COLUMN_WIDTH[column]);
  }
  if (spec.overlap) widths.push(COLUMN_WIDTH.overlap);
  return widths.reduce((sum, width) => sum + width + spec.gap, 0) + spec.padding;
}

/** Where the text of a row may start at the most: the graph area is cut there, never the
    right columns; `subjectRoom` is what the subject keeps before that. Never left of one lane. */
export function graphClipX(panelWidth: number, rightWidth: number, subjectRoom: number): number {
  return Math.max(textX(1), panelWidth - rightWidth - subjectRoom);
}

export function rowTextX(width: number, clipX: number): number {
  return Math.min(textX(width), clipX);
}

/** The right columns in the order asked for, the overlap cell after the time as before. */
export function rightCells(columns: readonly GraphColumn[], overlap: boolean): (GraphColumn | "overlap")[] {
  const cells: (GraphColumn | "overlap")[] = [];
  for (const column of columns) {
    cells.push(column);
    if (column === "time" && overlap) cells.push("overlap");
  }
  if (overlap && !columns.includes("time")) cells.unshift("overlap");
  return cells;
}

/** "Only if changed": a face only on the topmost row of a run by one author. */
export function avatarShown(changedOnly: boolean, email: string, above: string | undefined): boolean {
  return !changedOnly || above?.toLowerCase() !== email.toLowerCase();
}
