/** The graph holds its rows in blocks that come and go, so a selection of commits is kept as
    oids and a range is read off the rows between two of them. */
export interface RowSource {
  loadedIndexOf(oid: string | null): number | null;
  indexOf(oid: string): Promise<number | null>;
  entry(index: number): Promise<{ commit: { oid: string } } | undefined>;
}

/** A range or Select All longer than this is cut: every row of a long history would be a
    long wait for a selection nobody can act on whole. */
export const SELECTION_CAP = 5000;

async function rowOf(rows: RowSource, oid: string): Promise<number | null> {
  return rows.loadedIndexOf(oid) ?? (await rows.indexOf(oid));
}

async function oidsIn(rows: RowSource, from: number, to: number): Promise<string[]> {
  const oids: string[] = [];
  for (let at = from; at <= to; at++) {
    const row = await rows.entry(at);
    if (row) oids.push(row.commit.oid);
  }
  return oids;
}

/** The commits from `anchor` to `target` in display order, both ends included; the end
    nearest `target` is kept when the range is cut. Null when either is not in the graph. */
export async function oidsBetween(rows: RowSource, anchor: string, target: string): Promise<string[] | null> {
  const [a, b] = await Promise.all([rowOf(rows, anchor), rowOf(rows, target)]);
  if (a === null || b === null) return null;
  let lo = Math.min(a, b);
  let hi = Math.max(a, b);
  if (hi - lo + 1 > SELECTION_CAP) {
    if (b < a) hi = lo + SELECTION_CAP - 1;
    else lo = hi - SELECTION_CAP + 1;
  }
  return oidsIn(rows, lo, hi);
}

/** Select All: the first rows of the graph, down to the cap. */
export function firstOids(rows: RowSource, total: number): Promise<string[]> {
  return oidsIn(rows, 0, Math.min(total, SELECTION_CAP) - 1);
}
