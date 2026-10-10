import type { Hunk } from "$lib/ipc";

/** Throwing away every line of a file that had no old side removes the file itself. */
export function discardsWholeNewFile(
  diff: { oldTotal: number; newTotal: number; hunks: readonly Hunk[] },
  keys: ReadonlySet<string>,
): boolean {
  if (diff.oldTotal !== 0 || diff.newTotal === 0) return false;
  const inserts = diff.hunks.flatMap((hunk) => hunk.rows.flatMap((row) => (row.kind === "insert" ? [`i:${row.new}`] : [])));
  return inserts.length > 0 && inserts.every((key) => keys.has(key));
}
