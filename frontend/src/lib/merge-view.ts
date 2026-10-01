import type { Origin, Region } from "$lib/ipc";

/** One row of the preview: the three panels share a row grid, so a conflict lines up in all
    of them. Read-only: a conflict is decided in the Conflict Solver. */
export interface MergeRow {
  region: number;
  conflict: boolean;
  origin: Origin | null;
  ours: string | null;
  result: string | null;
  theirs: string | null;
}

function cell(lines: readonly string[], at: number): string | null {
  return lines[at] ?? null;
}

/** The Result of the preview is what the merge made on its own; a conflict it could not
    settle shows the base lines, marked as a conflict. */
export function previewRows(regions: readonly Region[]): MergeRow[] {
  const rows: MergeRow[] = [];
  regions.forEach((region, index) => {
    const result = region.kind === "clean" ? region.lines : region.base;
    const ours = region.kind === "clean" ? region.lines : region.ours;
    const theirs = region.kind === "clean" ? region.lines : region.theirs;
    const height = Math.max(1, ours.length, result.length, theirs.length);

    for (let at = 0; at < height; at++) {
      rows.push({
        region: index,
        conflict: region.kind === "conflict",
        origin: region.kind === "clean" ? region.origin : null,
        ours: cell(ours, at),
        result: cell(result, at),
        theirs: cell(theirs, at),
      });
    }
  });
  return rows;
}

export function conflictRows(rows: readonly MergeRow[]): number[] {
  const starts: number[] = [];
  let last = -1;
  rows.forEach((row, at) => {
    if (row.conflict && row.region !== last) {
      starts.push(at);
      last = row.region;
    }
  });
  return starts;
}

export function conflictCount(regions: readonly Region[]): number {
  return regions.filter((region) => region.kind === "conflict").length;
}

/** Regions the merge settled on its own. They are correct, and still worth a look. */
export function autoResolvedCount(regions: readonly Region[]): number {
  return regions.filter((region) => region.kind === "clean" && region.origin !== "unchanged")
    .length;
}

/** Settled by the parser, not by the line diff. Called out on its own: the rule is that
    it gets looked at before the merge is committed (doc/08-diff-engine.md §8). */
export function syntacticCount(regions: readonly Region[]): number {
  return regions.filter((region) => region.kind === "clean" && region.origin === "syntactic")
    .length;
}

export function nextConflict(
  at: readonly number[],
  current: number | null,
  direction: 1 | -1,
): number | null {
  if (at.length === 0) return null;
  if (current === null) return at[0] ?? null;

  if (direction === 1) {
    return at.find((row) => row > current) ?? at[0] ?? null;
  }
  return [...at].reverse().find((row) => row < current) ?? at.at(-1) ?? null;
}

/** F6 in the Diff panel of the main window, a merge on screen: the next or previous conflict
    while there is one that way, as a diff steps through its changes; past the last and
    before the first it is the panel walk's (11 §7). */
export function panelConflictStep(
  press: { key: string; ctrl: boolean; shift: boolean; alt: boolean },
  conflicts: readonly number[],
  at: number | null,
): 1 | -1 | null {
  if (press.key !== "F6" || press.ctrl || press.alt) return null;
  if (press.shift) return at !== null && conflicts.some((row) => row < at) ? -1 : null;
  return conflicts.some((row) => at === null || row > at) ? 1 : null;
}
