import type { BlockRows } from "./solver-geometry";
import type { SolverKind, SolverRegion } from "./ipc";

/** What a Take button puts into the Result for one hunk. */
export type TakeAction = "ours" | "theirs" | "oursTheirs" | "theirsOurs" | "base";

/** One stretch of the file that is not the same on every side: the solver's unit of work. */
export interface Hunk {
  /** The region's index, so ids run in document order. */
  id: number;
  kind: Exclude<SolverKind, "equal">;
  base: string[];
  ours: string[];
  theirs: string[];
  /** What the Result holds before any decision; for a conflict, the base lines. */
  initial: string[];
}

/** Where a hunk sits in the Result document, in lines. */
export interface Span {
  id: number;
  start: number;
  count: number;
}

/** Every line ends with a newline: the document of a file ends on one, and a save keeps it
    (the engine takes the final newline off again for a file that had none). */
export function linesToText(lines: readonly string[]): string {
  return lines.length === 0 ? "" : `${lines.join("\n")}\n`;
}

export function textToLines(text: string): string[] {
  if (text === "") return [];
  const lines = text.split("\n");
  if (text.endsWith("\n")) lines.pop();
  return lines;
}

export function equalLines(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((line, at) => line === b[at]);
}

export function buildResult(regions: readonly SolverRegion[]): { text: string; hunks: Hunk[]; spans: Span[] } {
  const hunks: Hunk[] = [];
  const spans: Span[] = [];
  const lines: string[] = [];
  regions.forEach((region, id) => {
    if (region.kind !== "equal") {
      hunks.push({
        id,
        kind: region.kind,
        base: [...region.base],
        ours: [...region.ours],
        theirs: [...region.theirs],
        initial: [...region.result],
      });
      spans.push({ id, start: lines.length, count: region.result.length });
    }
    lines.push(...region.result);
  });
  return { text: linesToText(lines), hunks, spans };
}

export function takeLines(hunk: Hunk, action: TakeAction): string[] {
  switch (action) {
    case "ours":
      return [...hunk.ours];
    case "theirs":
      return [...hunk.theirs];
    case "oursTheirs":
      return [...hunk.ours, ...hunk.theirs];
    case "base":
      return [...hunk.base];
    case "theirsOurs":
      return [...hunk.theirs, ...hunk.ours];
  }
}

/** A conflict is undecided while the Result holds its base lines. Derived from the text, not
    remembered: editing the block back to the base, or Undo, makes it undecided again. Only
    an explicit decision (`decided`, a Take Base) keeps the base lines as the answer. A hunk
    the merge settled by itself never was one. */
export function isUnresolved(hunk: Hunk, current: readonly string[], decided = false): boolean {
  return hunk.kind === "conflict" && !decided && equalLines(current, hunk.base);
}

export function conflictsLeftLabel(count: number): string {
  if (count === 0) return "No conflicts left";
  return count === 1 ? "1 conflict left" : `${count} conflicts left`;
}

/** The current lines of each hunk, read out of the Result by the spans. */
export function hunkTexts(lines: readonly string[], spans: readonly Span[]): Map<number, string[]> {
  return new Map(spans.map((span) => [span.id, lines.slice(span.start, span.start + span.count)]));
}

export interface Labels {
  ours: string;
  theirs: string;
}

/** The text to write. A conflict still undecided goes out as git writes it, with both sides
    between markers, so the file is conflicted in the editor of whoever opens it next. */
export function composeSave(
  lines: readonly string[],
  spans: readonly Span[],
  hunks: readonly Hunk[],
  labels: Labels,
  decided: ReadonlySet<number> = new Set(),
): string {
  const byId = new Map(hunks.map((hunk) => [hunk.id, hunk]));
  const out: string[] = [];
  let at = 0;
  for (const span of [...spans].sort((a, b) => a.start - b.start)) {
    const hunk = byId.get(span.id);
    if (!hunk) continue;
    const current = lines.slice(span.start, span.start + span.count);
    if (!isUnresolved(hunk, current, decided.has(span.id))) continue;
    out.push(...lines.slice(at, span.start));
    out.push(`<<<<<<< ${labels.ours}`, ...hunk.ours, "=======", ...hunk.theirs, `>>>>>>> ${labels.theirs}`);
    at = span.start + span.count;
  }
  out.push(...lines.slice(at));
  return linesToText(out);
}

/** Which gutter buttons make a difference: not the ones that would put back what is there. */
export function visibleActions(hunk: Hunk, current: readonly string[]): { ours: boolean; theirs: boolean } {
  return { ours: !equalLines(current, hunk.ours), theirs: !equalLines(current, hunk.theirs) };
}

/** The next or previous of `ids` (document order) from `current`, wrapping around. */
export function nextHunk(ids: readonly number[], current: number | null, direction: 1 | -1): number | null {
  if (ids.length === 0) return null;
  if (current === null) return (direction === 1 ? ids[0] : ids.at(-1)) ?? null;
  if (direction === 1) return ids.find((id) => id > current) ?? ids[0] ?? null;
  return [...ids].reverse().find((id) => id < current) ?? ids.at(-1) ?? null;
}

export type Tone = "add" | "del" | "changed";

/** How Ours or Theirs colors a hunk. Without Base Changes: where the two sides differ from
    each other. With it: what that side did to the base. */
export function paneTone(hunk: Hunk, side: "ours" | "theirs", baseChanges: boolean): Tone | null {
  const lines = hunk[side];
  if (!baseChanges) return equalLines(hunk.ours, hunk.theirs) ? null : "changed";
  if (equalLines(lines, hunk.base)) return null;
  if (hunk.base.length === 0) return "add";
  return lines.length === 0 ? "del" : "changed";
}

/** Where each hunk sits in the document of one side, in lines. */
export function sideRows(
  regions: readonly SolverRegion[],
  side: "ours" | "theirs" | "base",
): { start: number; count: number }[] {
  const rows: { start: number; count: number }[] = [];
  let at = 0;
  for (const region of regions) {
    const lines = region[side].length;
    if (region.kind !== "equal") rows.push({ start: at, count: lines });
    at += lines;
  }
  return rows;
}

export interface SolverDocs {
  oursText: string;
  theirsText: string;
  /** What the Result starts as. */
  resultText: string;
  hunks: Hunk[];
  spans: Span[];
  oursRows: BlockRows[];
  theirsRows: BlockRows[];
}

function sideText(regions: readonly SolverRegion[], side: "ours" | "theirs"): string {
  return linesToText(regions.flatMap((region) => region[side]));
}

export function docsFromRegions(regions: readonly SolverRegion[]): SolverDocs {
  const built = buildResult(regions);
  return {
    oursText: sideText(regions, "ours"),
    theirsText: sideText(regions, "theirs"),
    resultText: built.text,
    hunks: built.hunks,
    spans: built.spans,
    oursRows: sideRows(regions, "ours"),
    theirsRows: sideRows(regions, "theirs"),
  };
}

/** Modify/delete: the side that lost the file is empty, the other is what the Result starts
    from, and nothing is merged line by line. */
export function docsForDeleted(sides: { ours: string | null; theirs: string | null }): SolverDocs {
  const kept = sides.ours ?? sides.theirs ?? "";
  return {
    oursText: sides.ours ?? "",
    theirsText: sides.theirs ?? "",
    resultText: kept,
    hunks: [],
    spans: [],
    oursRows: [],
    theirsRows: [],
  };
}
