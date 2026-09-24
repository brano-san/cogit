import { relativeDate, smartDate } from "$lib/format";
import { GRAPH_COLUMNS, type GraphColumn, type GraphTimeFormat } from "$lib/settings";

export const COLUMN_LABELS: Record<GraphColumn, string> = {
  author: "Author name",
  avatar: "Avatar",
  time: "Time",
  hash: "Hash",
};

export interface ColumnRow {
  id: GraphColumn;
  shown: boolean;
}

/** Every column, the visible ones first in their order, then the hidden ones. */
export function columnRows(visible: readonly GraphColumn[]): ColumnRow[] {
  return [
    ...visible.map((id) => ({ id, shown: true })),
    ...GRAPH_COLUMNS.filter((id) => !visible.includes(id)).map((id) => ({ id, shown: false })),
  ];
}

export function visibleColumns(rows: readonly ColumnRow[]): GraphColumn[] {
  return rows.filter((row) => row.shown).map((row) => row.id);
}

/** The editor keeps where a hidden column sits while it is open; once the visible ones
    change from outside (Restore Defaults, Cancel), that arrangement is gone. */
export function reconcileRows(rows: readonly ColumnRow[], visible: readonly GraphColumn[]): ColumnRow[] {
  const same =
    rows.length === GRAPH_COLUMNS.length &&
    visibleColumns(rows).join() === visible.join();
  return same ? [...rows] : columnRows(visible);
}

export function toggleColumn(rows: readonly ColumnRow[], id: GraphColumn): ColumnRow[] {
  return rows.map((row) => (row.id === id ? { ...row, shown: !row.shown } : row));
}

export function moveColumn(rows: readonly ColumnRow[], from: number, to: number): ColumnRow[] {
  if (from === to || from < 0 || from >= rows.length) return [...rows];
  const target = Math.min(Math.max(to, 0), rows.length - 1);
  const next = [...rows];
  const [moved] = next.splice(from, 1);
  if (moved) next.splice(target, 0, moved);
  return next;
}

const pad = (n: number) => String(n).padStart(2, "0");

/** `date` is what the column always showed: today, yesterday, the weekday, then DD-MM-YY. */
export function graphTime(
  timestamp: number,
  offsetMinutes: number,
  now: number,
  format: GraphTimeFormat,
): string {
  if (format === "relative") return relativeDate(timestamp, offsetMinutes, now);
  const date = smartDate(timestamp, offsetMinutes, now);
  if (format === "date") return date;
  const shifted = new Date((timestamp + offsetMinutes * 60) * 1000);
  return `${date} ${pad(shifted.getUTCHours())}:${pad(shifted.getUTCMinutes())}`;
}
