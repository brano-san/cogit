import type { DiffRow, Hunk } from "$lib/ipc";

/** `d:` and `i:` keep the two sides apart: line 3 old and line 3 new are different lines. */
export function lineKey(row: DiffRow): string | null {
  if (row.kind === "delete") return `d:${row.old}`;
  if (row.kind === "insert") return `i:${row.new}`;
  return null;
}

export function toggleLine(selected: ReadonlySet<string>, key: string): Set<string> {
  const next = new Set(selected);
  if (!next.delete(key)) next.add(key);
  return next;
}

export function hunkSelection(hunk: Hunk): Set<string> {
  const keys = new Set<string>();
  for (const row of hunk.rows) {
    const key = lineKey(row);
    if (key) keys.add(key);
  }
  return keys;
}

export function splitSelection(selected: ReadonlySet<string>): {
  deletes: number[];
  inserts: number[];
} {
  const deletes: number[] = [];
  const inserts: number[] = [];
  for (const key of selected) {
    const line = Number(key.slice(2));
    if (key.startsWith("d:")) deletes.push(line);
    else inserts.push(line);
  }
  return { deletes, inserts };
}
