import { scrollMarks } from "$lib/solver-geometry";

export interface RowMark<T extends string> {
  id: number;
  top: number;
  height: number;
  tone: T;
}

export function rowMarks<T extends string>(tones: readonly (T | "")[], rowHeight: number, viewport: number): RowMark<T>[] {
  const runs: { start: number; end: number; tone: T }[] = [];
  tones.forEach((tone, at) => {
    if (tone === "") return;
    const last = runs.at(-1);
    if (last && last.end === at && last.tone === tone) last.end = at + 1;
    else runs.push({ start: at, end: at + 1, tone });
  });
  const edges = runs.map((run) => ({ top: run.start * rowHeight, bottom: run.end * rowHeight }));
  return scrollMarks(edges, tones.length * rowHeight, viewport).flatMap((mark) => {
    const run = runs[mark.at];
    return run ? [{ id: run.start, top: mark.top, height: mark.height, tone: run.tone }] : [];
  });
}
