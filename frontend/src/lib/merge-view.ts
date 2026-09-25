import type { Origin, Region } from "$lib/ipc";

export type Choice = "ours" | "theirs" | "both";

/** Region index to the side the user picked. A conflict without an entry is undecided. */
export type Choices = Record<number, Choice>;

export interface MergeRow {
  region: number;
  conflict: boolean;
  origin: Origin | null;
  base: string | null;
  ours: string | null;
  theirs: string | null;
  result: string | null;
}

function resolvedLines(region: Region, choice: Choice | undefined): string[] | null {
  if (region.kind === "clean") return region.lines;
  if (choice === "ours") return region.ours;
  if (choice === "theirs") return region.theirs;
  if (choice === "both") return [...region.ours, ...region.theirs];
  return null;
}

function cell(lines: string[], at: number): string | null {
  return lines[at] ?? null;
}

/** The four panels share one row grid, so a conflict lines up across all of them. */
export function mergeRows(regions: readonly Region[], choices: Choices): MergeRow[] {
  const rows: MergeRow[] = [];
  regions.forEach((region, index) => {
    const result = resolvedLines(region, choices[index]);
    const base = region.kind === "clean" ? region.lines : region.base;
    const ours = region.kind === "clean" ? region.lines : region.ours;
    const theirs = region.kind === "clean" ? region.lines : region.theirs;
    const height = Math.max(1, base.length, ours.length, theirs.length, result?.length ?? 0);

    for (let at = 0; at < height; at++) {
      rows.push({
        region: index,
        conflict: region.kind === "conflict",
        origin: region.kind === "clean" ? region.origin : null,
        base: cell(base, at),
        ours: cell(ours, at),
        theirs: cell(theirs, at),
        result: result === null ? null : cell(result, at),
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

export function unresolvedCount(regions: readonly Region[], choices: Choices): number {
  return regions.filter((region, index) => region.kind === "conflict" && !choices[index]).length;
}

export function chooseAll(regions: readonly Region[], side: Choice): Choices {
  const choices: Choices = {};
  regions.forEach((region, index) => {
    if (region.kind === "conflict") choices[index] = side;
  });
  return choices;
}

export function mergedText(regions: readonly Region[], choices: Choices): string {
  const lines = regions.flatMap((region, index) => {
    const resolved = resolvedLines(region, choices[index]);
    if (resolved !== null) return resolved;
    return region.kind === "conflict" ? region.base : [];
  });
  return lines.length === 0 ? "" : `${lines.join("\n")}\n`;
}

/** What the hand editor starts from: an undecided conflict keeps both sides between the
    markers git writes, so editing by hand cannot drop either side without seeing it. */
export function editableText(regions: readonly Region[], choices: Choices): string {
  const lines = regions.flatMap((region, index) => {
    const resolved = resolvedLines(region, choices[index]);
    if (resolved !== null || region.kind !== "conflict") return resolved ?? [];
    return [
      "<<<<<<< ours",
      ...region.ours,
      "||||||| base",
      ...region.base,
      "=======",
      ...region.theirs,
      ">>>>>>> theirs",
    ];
  });
  return lines.length === 0 ? "" : `${lines.join("\n")}\n`;
}

const MARKER = /^(<{7}|\|{7}|>{7})( |$)|^={7}$/m;

/** The panels save once every conflict has a side; hand-edited text once no marker is left. */
export function canSave(regions: readonly Region[], choices: Choices, edited: string | null): boolean {
  if (edited === null) return unresolvedCount(regions, choices) === 0;
  return !MARKER.test(edited.replaceAll("\r\n", "\n"));
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
