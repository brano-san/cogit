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

/**
 * The line range a selection covers, for Investigate.
 *
 * `git log -L` counts lines in the file as it stands, so the new side wins whenever the
 * selection has one. A selection of deletions alone falls back to the old side, which is
 * the only numbering those lines ever had.
 */
export function selectedRange(
  selected: ReadonlySet<string>,
): { from: number; to: number } | null {
  const { deletes, inserts } = splitSelection(selected);
  const lines = inserts.length > 0 ? inserts : deletes;
  if (lines.length === 0) return null;
  return { from: Math.min(...lines), to: Math.max(...lines) };
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
