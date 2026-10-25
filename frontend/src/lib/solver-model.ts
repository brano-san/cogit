import type { BlockRows } from "./solver-geometry";
import type { SideName, SolverKind, SolverRegion } from "./ipc";

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
  /** What the Result holds before any decision; for a conflict, both sides between markers. */
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

export interface Labels {
  ours: string;
  theirs: string;
}

const NO_LABELS: Labels = { ours: "ours", theirs: "theirs" };

/** A conflict as git writes it into the file: the Result shows it so, and a save keeps it so. */
export function markerLines(ours: readonly string[], theirs: readonly string[], labels: Labels): string[] {
  return [`<<<<<<< ${labels.ours}`, ...ours, "=======", ...theirs, `>>>>>>> ${labels.theirs}`];
}

const MARKER = /^(<{7} |={7}$|>{7} )/;

export function buildResult(
  regions: readonly SolverRegion[],
  labels: Labels = NO_LABELS,
): { text: string; hunks: Hunk[]; spans: Span[] } {
  const hunks: Hunk[] = [];
  const spans: Span[] = [];
  const lines: string[] = [];
  regions.forEach((region, id) => {
    const result = region.kind === "conflict" ? markerLines(region.ours, region.theirs, labels) : region.result;
    if (region.kind !== "equal") {
      hunks.push({
        id,
        kind: region.kind,
        base: [...region.base],
        ours: [...region.ours],
        theirs: [...region.theirs],
        initial: [...result],
      });
      spans.push({ id, start: lines.length, count: result.length });
    }
    lines.push(...result);
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

/** A conflict is undecided while its stretch of the Result still holds a conflict marker and
    nobody decided it. Any Take marks it `decided` and takes the markers out with the rest;
    only Undo of the Take removes the mark (`invertedEffects`). A hunk the merge settled by
    itself never was one. */
export function isUnresolved(hunk: Hunk, current: readonly string[], decided = false): boolean {
  return hunk.kind === "conflict" && !decided && current.some((line) => MARKER.test(line));
}

export function conflictsLeftLabel(count: number): string {
  if (count === 0) return "No conflicts left";
  return count === 1 ? "1 conflict left" : `${count} conflicts left`;
}

/** The current lines of each hunk, read out of the Result by the spans. */
export function hunkTexts(lines: readonly string[], spans: readonly Span[]): Map<number, string[]> {
  return new Map(spans.map((span) => [span.id, lines.slice(span.start, span.start + span.count)]));
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

export type Tone = "add" | "del" | "changed" | "conflict" | "resolved";

/** A conflict is its own color, undecided or decided, whatever the sides did. Any other hunk:
    with Base Changes, what that side did to the base (`paneTone`); without, how the Result
    differs from that side (`sideTone`), so a side the Result already is has no color. */
export function hunkTone(
  hunk: Hunk,
  side: "ours" | "theirs",
  baseChanges: boolean,
  unresolved: ReadonlySet<number>,
  current?: readonly string[],
): Tone | null {
  if (hunk.kind === "conflict") return unresolved.has(hunk.id) ? "conflict" : "resolved";
  if (!baseChanges && current) return sideTone(hunk[side], current);
  return paneTone(hunk, side, baseChanges);
}

/** The Result's lines of a hunk against a side's: what was added, deleted or changed. */
export function sideTone(side: readonly string[], current: readonly string[]): Tone | null {
  if (equalLines(side, current)) return null;
  if (side.length === 0) return "add";
  return current.length === 0 ? "del" : "changed";
}

/** What a block of the Result is, in words for its tooltip (the legend). */
export function hunkLegend(hunk: Hunk, unresolved: boolean): string {
  // Only a side that lost the whole file is empty where the base is not and the other
  // side alone changed (`regionsForMissing`): a real one-sided hunk leaves its side as the base.
  if (hunk.kind !== "conflict" && hunk.base.length > 0) {
    if (hunk.kind === "theirs" && hunk.ours.length === 0) return "Ours deleted this file: here it is as Theirs has it.";
    if (hunk.kind === "ours" && hunk.theirs.length === 0) return "Theirs deleted this file: here it is as Ours has it.";
  }
  switch (hunk.kind) {
    case "conflict":
      return unresolved
        ? "Conflict: both sides changed these lines. Take Ours, Take Theirs, both, or edit them."
        : "Resolved conflict: decided here.";
    case "ours":
      return "Merged automatically: only Ours changed these lines.";
    case "theirs":
      return "Merged automatically: only Theirs changed these lines.";
    case "both":
      return "Merged automatically: both sides made the same change.";
    case "syntactic":
      return "Merged automatically: both sides changed different parts of the code here.";
  }
}

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

export function docsFromRegions(regions: readonly SolverRegion[], labels: Labels = NO_LABELS): SolverDocs {
  const built = buildResult(regions, labels);
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

export interface SolverSides {
  base: string | null;
  ours: string | null;
  theirs: string | null;
  regions: readonly SolverRegion[];
}

/** Modify/delete: the side that lost the file is an empty one, and the whole file is one
    block the Result starts from as the side that kept it has it. Nothing is folded away. */
function regionsForMissing(sides: SolverSides): SolverRegion[] {
  const base = textToLines(sides.base ?? "");
  const ours = textToLines(sides.ours ?? "");
  const theirs = textToLines(sides.theirs ?? "");
  const kept = sides.ours === null ? theirs : ours;
  if (equalLines(ours, theirs)) return [{ kind: "equal", base, ours, theirs, result: kept }];
  return [{ kind: sides.ours === null ? "theirs" : "ours", base, ours, theirs, result: kept }];
}

export function solverDocs(sides: SolverSides, labels: Labels = NO_LABELS): SolverDocs {
  const missing = sides.ours === null || sides.theirs === null || sides.regions.length === 0;
  return docsFromRegions(missing ? regionsForMissing(sides) : sides.regions, labels);
}

/** Below this nothing is folded: the whole file is on screen anyway. */
const SMALL_FILE_LINES = 50;

export function solverContext(context: number, docs: SolverDocs): number {
  const longest = Math.max(
    ...[docs.oursText, docs.resultText, docs.theirsText].map((text) => textToLines(text).length),
  );
  return longest <= SMALL_FILE_LINES ? Number.POSITIVE_INFINITY : context;
}

/** A pane's header: its part in the operation, which side git calls it and its commit;
    the subject goes to the tooltip. `Rebase target ("ours", e059661) — deleted`. */
export function paneTitle(side: SideName, which: "ours" | "theirs", deleted: boolean): string {
  const oid = side.oid ? `, ${side.oid}` : "";
  return `${side.role} ("${which}"${oid})${deleted ? " — deleted" : ""}`;
}

export function paneTooltip(side: SideName, which: "ours" | "theirs", deleted: boolean): string {
  const title = paneTitle(side, which, deleted);
  return side.subject ? `${title}\n${side.subject}` : title;
}

/** Which Take actions would change a hunk's lines as they are now: the toolbar offers
    only those (a side the Result already is has nothing to take). */
export function takeChanges(hunk: Hunk, current: readonly string[]): Record<TakeAction, boolean> {
  const actions: TakeAction[] = ["ours", "theirs", "oursTheirs", "theirsOurs", "base"];
  return Object.fromEntries(actions.map((action) => [action, !equalLines(current, takeLines(hunk, action))])) as Record<
    TakeAction,
    boolean
  >;
}
