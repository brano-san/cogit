import { bisectPhase } from "./bisect";
import type { BisectState } from "./ipc/bisect";

/* The bisect layer of the graph (F-566): over the rows, apart from the paint of their lines
   (`graph_engine::paint`), so no coloring is changed by it. */

/** How a commit row shows in the graph: a dot for its mark, a tint for the row. */
export interface BisectLook {
  dot: "good" | "bad" | "skip" | null;
  row: "current" | "found" | null;
  /** In the row before its labels: a colour alone is no answer for everyone. */
  tag: string;
}

const DOT_TOKENS = {
  good: "--graph-bisect-good",
  bad: "--graph-bisect-bad",
  skip: "--graph-bisect-skip",
} as const;

const ROW_TOKENS = {
  current: "--graph-bisect-current",
  found: "--graph-bisect-found",
} as const;

export function dotToken(look: BisectLook | undefined): string | null {
  return look?.dot ? DOT_TOKENS[look.dot] : null;
}

export function rowToken(look: BisectLook | undefined): string | null {
  return look?.row ? ROW_TOKENS[look.row] : null;
}

/** One entry per commit the bisect knows; a later rule overrides an earlier one. */
export function bisectLooks(bisect: BisectState | null): ReadonlyMap<string, BisectLook> {
  const looks = new Map<string, BisectLook>();
  if (!bisect) return looks;
  const put = (oid: string, change: Partial<BisectLook>) => {
    const was = looks.get(oid) ?? { dot: null, row: null, tag: "" };
    looks.set(oid, { ...was, ...change });
  };
  for (const oid of bisect.skipped) put(oid, { dot: "skip", tag: "skipped" });
  for (const oid of bisect.good) put(oid, { dot: "good", tag: bisect.terms.good });
  if (bisect.bad) put(bisect.bad, { dot: "bad", tag: bisect.terms.bad });
  const phase = bisectPhase(bisect);
  if (phase === "found" && bisect.firstBad) {
    put(bisect.firstBad, { dot: "bad", row: "found", tag: `first ${bisect.terms.bad}` });
  } else if (phase === "stuck") {
    for (const oid of bisect.candidates) put(oid, { row: "found", tag: "candidate" });
  } else if (bisect.current) {
    const mark = looks.get(bisect.current);
    put(bisect.current, { row: "current", tag: mark?.tag ? mark.tag : "testing" });
  }
  return looks;
}
