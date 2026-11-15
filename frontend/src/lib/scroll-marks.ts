import { scrollMarks } from "$lib/solver-geometry";
import { uniformRows, type RowGeometry } from "$lib/diff-wrap";

export interface RowMark<T extends string> {
  id: number;
  top: number;
  height: number;
  tone: T;
}

/** `rows`: one row height, or the rows' own heights when lines wrap (F-950). */
export function rowMarks<T extends string>(
  tones: readonly (T | "")[],
  rows: number | RowGeometry,
  viewport: number,
): RowMark<T>[] {
  const geometry = typeof rows === "number" ? uniformRows(tones.length, rows) : rows;
  const runs: { start: number; end: number; tone: T }[] = [];
  tones.forEach((tone, at) => {
    if (tone === "") return;
    const last = runs.at(-1);
    if (last && last.end === at && last.tone === tone) last.end = at + 1;
    else runs.push({ start: at, end: at + 1, tone });
  });
  const edges = runs.map((run) => ({ top: geometry.top(run.start), bottom: geometry.top(run.end) }));
  return scrollMarks(edges, geometry.top(tones.length), viewport).flatMap((mark) => {
    const run = runs[mark.at];
    return run ? [{ id: run.start, top: mark.top, height: mark.height, tone: run.tone }] : [];
  });
}
