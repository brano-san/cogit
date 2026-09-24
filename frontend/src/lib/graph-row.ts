import { displayDate, relativeDate, shortDate, type DateMode } from "$lib/format";
import { textX } from "$lib/graph-geometry";

/** A commit row of the graph, driven by the graph settings (#12). Right columns are pinned
    to the right edge; the middle gives way, the branch labels first, then the subject; past
    that the graph area is cut at its edge rather than its lanes squeezed (R-331). */

export type GraphColumn = "hash" | "author" | "avatar" | "time";
export type GraphTimeFormat = "relative" | "date" | "dateTime";
export type GraphDensity = "compact" | "normal" | "comfortable";

/** How the list looks until Preferences say otherwise: as it always has. */
export const GRAPH_COLUMNS: readonly GraphColumn[] = ["author", "avatar", "time", "hash"];
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

/** Wide enough for the longest value the format writes, in the 11 px the column uses. */
export function timeWidth(format: GraphTimeFormat | DateMode): number {
  switch (format) {
    case "date":
      return 56;
    case "smart":
      return 74;
    case "relative":
      return 84;
    case "dateTime":
      return 96;
    case "both":
      return 150;
  }
}

/** The app's own date modes (`smart`, `both`) are what the list shows until the graph has one. */
export function graphTime(
  timestamp: number,
  offsetMinutes: number,
  now: number,
  format: GraphTimeFormat | DateMode,
): string {
  switch (format) {
    case "relative":
      return relativeDate(timestamp, offsetMinutes, now);
    case "date":
      return shortDate(timestamp, offsetMinutes);
    case "dateTime": {
      const shifted = new Date((timestamp + offsetMinutes * 60) * 1000);
      const pad = (n: number) => String(n).padStart(2, "0");
      return `${shortDate(timestamp, offsetMinutes)} ${pad(shifted.getUTCHours())}:${pad(shifted.getUTCMinutes())}`;
    }
    default:
      return displayDate(timestamp, offsetMinutes, now, format);
  }
}

export interface RightColumns {
  columns: readonly GraphColumn[];
  /** Avatars off draw nothing, so the column takes no room. */
  avatars: boolean;
  time: GraphTimeFormat | DateMode;
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
