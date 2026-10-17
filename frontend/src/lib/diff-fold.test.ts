import { describe, expect, it } from "vitest";
import type { DiffRow, Hunk } from "./ipc/bindings";
import {
  openingRow,
  changeSteps,
  foldContext,
  foldDiff,
  highlightedRows,
  missingLines,
  navState,
  revealRange,
  stepsAround,
  type FoldEntry,
  type Gap,
} from "./diff-fold";

const c = (old: number, next: number, text = `    line ${old}`): DiffRow => ({
  kind: "context",
  old,
  new: next,
  text,
});
const d = (old: number, text = `    old ${old}`): DiffRow => ({ kind: "delete", old, text, inline: [] });
const i = (next: number, text = `    new ${next}`): DiffRow => ({ kind: "insert", new: next, text, inline: [] });

function hunk(rows: DiffRow[], header = ""): Hunk {
  const olds = rows.flatMap((r) => (r.kind === "context" || r.kind === "delete" ? [r.old] : []));
  const news = rows.flatMap((r) => (r.kind === "context" || r.kind === "insert" ? [r.new] : []));
  return {
    oldStart: olds.length ? Math.min(...olds) : 0,
    oldLines: olds.length,
    newStart: news.length ? Math.min(...news) : 0,
    newLines: news.length,
    header: header || `@@ -${Math.min(...olds)},${olds.length} +${Math.min(...news)},${news.length} @@`,
    rows,
  };
}

/** Lines `from..=to` as context, with the new side shifted by `shift`. */
function context(from: number, to: number, shift = 0): DiffRow[] {
  const rows: DiffRow[] = [];
  for (let n = from; n <= to; n++) rows.push(c(n, n + shift));
  return rows;
}

function gaps(entries: FoldEntry[]): Gap[] {
  return entries.flatMap((entry) => (entry.kind === "gap" ? [entry.gap] : []));
}

function shown(entries: FoldEntry[]): (number | string)[] {
  return entries.map((entry) => {
    if (entry.kind === "gap") return entry.gap.hidden === 0 ? "strip" : `gap ${entry.gap.oldFrom}-${entry.gap.oldTo}`;
    const row = entry.row;
    return row.kind === "insert" ? `+${row.new}` : row.kind === "delete" ? `-${row.old}` : row.kind === "context" ? row.old : "?";
  });
}

const BASE = { context: 3, revealed: [] };

describe("foldDiff: gaps between the hunks the backend sent", () => {
  it("puts a gap above a hunk that does not start at line 1, with the function it is in", () => {
    const h = hunk([...context(7, 9), d(10), i(10), ...context(11, 13)], "@@ -7,7 +7,7 @@ struct ElrsChannel {");
    const entries = foldDiff({ ...BASE, hunks: [h], oldTotal: 13, newTotal: 13 });

    expect(gaps(entries)).toEqual([
      {
        oldFrom: 1,
        oldTo: 6,
        newFrom: 1,
        hidden: 6,
        loaded: false,
        up: true,
        down: false,
        context: "struct ElrsChannel {",
      },
    ]);
  });

  it("puts a gap between two hunks and after the last one", () => {
    const first = hunk([...context(1, 3), d(4), ...context(5, 7)]);
    const second = hunk([...context(20, 22, -1), i(22), ...context(23, 25, 0)]);
    const entries = foldDiff({ ...BASE, hunks: [first, second], oldTotal: 40, newTotal: 40 });

    expect(gaps(entries).map((g) => [g.oldFrom, g.oldTo, g.newFrom, g.up, g.down])).toEqual([
      [8, 19, 7, true, true],
      [26, 40, 26, false, true],
    ]);
  });

  it("draws no gap at all for a new file", () => {
    const h = hunk([i(1), i(2), i(3)]);
    const entries = foldDiff({ ...BASE, hunks: [h], oldTotal: 0, newTotal: 3 });

    expect(gaps(entries)).toEqual([]);
    expect(entries).toHaveLength(3);
  });

  it("draws no gap at all for a deleted file", () => {
    const h = hunk([d(1), d(2)]);
    expect(gaps(foldDiff({ ...BASE, hunks: [h], oldTotal: 2, newTotal: 0 }))).toEqual([]);
  });

  // DF-006: with no context lines git writes an empty side as the line it follows,
  // `@@ -5,0 +6,2 @@` for two lines inserted after line 5.
  it("folds the lines around a change that has an empty side", () => {
    const inserted: Hunk = { oldStart: 5, oldLines: 0, newStart: 6, newLines: 2, header: "", rows: [i(6), i(7)] };
    expect(
      gaps(foldDiff({ ...BASE, context: 0, hunks: [inserted], oldTotal: 10, newTotal: 12 })).map((g) => [
        g.oldFrom,
        g.oldTo,
        g.newFrom,
        g.hidden,
      ]),
    ).toEqual([
      [1, 5, 1, 5],
      [6, 10, 8, 5],
    ]);

    const deleted: Hunk = { oldStart: 6, oldLines: 2, newStart: 5, newLines: 0, header: "", rows: [d(6), d(7)] };
    expect(
      gaps(foldDiff({ ...BASE, context: 0, hunks: [deleted], oldTotal: 12, newTotal: 10 })).map((g) => [
        g.oldFrom,
        g.oldTo,
        g.newFrom,
        g.hidden,
      ]),
    ).toEqual([
      [1, 5, 1, 5],
      [8, 12, 6, 5],
    ]);
  });

  it("numbers the blocks between gaps", () => {
    const first = hunk([...context(1, 3), d(4), ...context(5, 7)]);
    const second = hunk([...context(20, 22, -1), i(22), ...context(23, 25)]);
    const entries = foldDiff({ ...BASE, hunks: [first, second], oldTotal: 25, newTotal: 25 });

    const blocks = entries.flatMap((e) => (e.kind === "row" ? [e.block] : []));
    expect(new Set(blocks)).toEqual(new Set([0, 1]));
  });
});

describe("foldDiff: folding a diff that carries the whole file", () => {
  const whole = hunk([...context(1, 20), d(21), i(21), ...context(22, 60)]);

  it("keeps three lines around the change and folds the rest", () => {
    const entries = foldDiff({ ...BASE, hunks: [whole], oldTotal: 60, newTotal: 60 });

    expect(shown(entries)).toEqual(["gap 1-17", 18, 19, 20, "-21", "+21", 22, 23, 24, "gap 25-60"]);
    expect(gaps(entries).every((g) => g.loaded)).toBe(true);
  });

  it("shows a fold of one or two lines instead of folding it", () => {
    const small = hunk([...context(1, 5), d(6), ...context(7, 14), i(14), ...context(15, 17)]);
    const entries = foldDiff({ ...BASE, hunks: [small], oldTotal: 17, newTotal: 17 });

    expect(gaps(entries)).toEqual([]);
  });

  it("keeps what the user revealed, splitting a fold in two", () => {
    const entries = foldDiff({
      ...BASE,
      hunks: [whole],
      oldTotal: 60,
      newTotal: 60,
      revealed: [{ from: 35, to: 40 }],
    });

    expect(gaps(entries).map((g) => [g.oldFrom, g.oldTo])).toEqual([
      [1, 17],
      [25, 34],
      [41, 60],
    ]);
  });

  it("names the declaration above a folded gap, the way git names a hunk", () => {
    const rows = [
      c(1, 1, "fn first() {"),
      ...context(2, 30),
      c(31, 31, "impl Second {"),
      ...context(32, 50),
      d(51),
      ...context(52, 54),
    ];
    const entries = foldDiff({ ...BASE, hunks: [hunk(rows)], oldTotal: 54, newTotal: 53 });

    expect(gaps(entries)[0]?.context).toBe("impl Second {");
  });

  it("gives the gap above nothing declared no context", () => {
    const entries = foldDiff({ ...BASE, hunks: [whole], oldTotal: 60, newTotal: 60 });
    expect(gaps(entries)[0]?.context).toBeNull();
  });
});

// Stage, Unstage and Discard act on the block below the fold that carries them. Without line
// selection (R-748) a block with no fold above it — at the top of the file, in a new file, in
// a file shown whole — could not be staged from the diff at all.
describe("foldDiff: a strip above every block of changes, for its actions", () => {
  const EVERY = { from: 1, to: Number.MAX_SAFE_INTEGER };
  /** The block the deleted line `old` is in. */
  const blockOf = (entries: FoldEntry[], old: number) =>
    entries.flatMap((e) => (e.kind === "row" && e.row.kind === "delete" && e.row.old === old ? [e.block] : []))[0];

  it("puts one at the top when no fold is above the first change", () => {
    const h = hunk([...context(1, 2), d(3), i(3), ...context(4, 6)]);
    const entries = foldDiff({ ...BASE, hunks: [h], oldTotal: 6, newTotal: 6, strips: true });

    expect(shown(entries)).toEqual(["strip", 1, 2, "-3", "+3", 4, 5, 6]);
  });

  it("puts one above the lines of a new file", () => {
    const entries = foldDiff({ ...BASE, hunks: [hunk([i(1), i(2)])], oldTotal: 0, newTotal: 2, strips: true });

    expect(shown(entries)).toEqual(["strip", "+1", "+2"]);
  });

  it("adds none where a fold carries them", () => {
    const h = hunk([...context(7, 9), d(10), i(10), ...context(11, 13)]);
    const entries = foldDiff({ ...BASE, hunks: [h], oldTotal: 13, newTotal: 13, strips: true });

    expect(shown(entries)).toEqual(["gap 1-6", 7, 8, 9, "-10", "+10", 11, 12, 13]);
  });

  it("splits a file shown whole where git splits hunks, as many lines above a block as the context", () => {
    const rows = [
      ...context(1, 4),
      d(5),
      ...context(6, 11, -1),
      d(12),
      ...context(13, 19, -2),
      d(20),
      ...context(21, 23, -3),
    ];
    const entries = foldDiff({ ...BASE, hunks: [hunk(rows)], oldTotal: 23, newTotal: 20, revealed: [EVERY], strips: true });

    expect(shown(entries)).toEqual([
      1, "strip", 2, 3, 4, "-5", 6, 7, 8, 9, 10, 11, "-12", 13, 14, 15, 16, "strip", 17, 18, 19, "-20", 21, 22, 23,
    ]);
    expect(blockOf(entries, 5)).toBe(blockOf(entries, 12));
    expect(blockOf(entries, 20)).not.toBe(blockOf(entries, 12));
    expect(gaps(entries)[1]).toEqual({
      oldFrom: 17,
      oldTo: 16,
      newFrom: 15,
      hidden: 0,
      loaded: true,
      up: false,
      down: false,
      context: null,
    });
  });

  it("puts one right above a block when no line of context is kept", () => {
    const rows = [d(1), ...context(2, 3, -1), d(4), ...context(5, 5, -2)];
    const entries = foldDiff({ ...BASE, context: 0, hunks: [hunk(rows)], oldTotal: 5, newTotal: 3, revealed: [EVERY], strips: true });

    expect(shown(entries)).toEqual(["strip", "-1", 2, 3, "strip", "-4", 5]);
  });

  it("draws none where nothing can be staged: the diff of a commit", () => {
    const h = hunk([...context(1, 2), d(3), i(3), ...context(4, 6)]);

    expect(shown(foldDiff({ ...BASE, hunks: [h], oldTotal: 6, newTotal: 6 }))).toEqual([1, 2, "-3", "+3", 4, 5, 6]);
  });
});

// DF-049: opening one band loads the whole file; past the parser's limit that turned the
// colour off for every line of the diff.
describe("highlightedRows", () => {
  const whole = hunk([...context(1, 20), d(21), i(21), ...context(22, 60)]);
  const shownRows = foldDiff({ ...BASE, hunks: [whole], oldTotal: 60, newTotal: 60 });

  it("takes every loaded row while each side fits", () => {
    let asked = false;
    const rows = highlightedRows([whole], () => ((asked = true), shownRows), 100);
    expect(rows).toHaveLength(61);
    expect(asked).toBe(false);
  });

  it("takes only the rows on show once a side is past the limit", () => {
    const rows = highlightedRows([whole], () => shownRows, 30);
    expect(rows.map((row) => (row.kind === "context" ? row.old : row.kind))).toEqual([
      18, 19, 20, "delete", "insert", 22, 23, 24,
    ]);
  });
});

describe("revealRange", () => {
  const gap: Gap = {
    oldFrom: 10,
    oldTo: 69,
    newFrom: 12,
    hidden: 60,
    loaded: true,
    up: true,
    down: true,
    context: null,
  };

  it("▲ shows the twenty lines just above the change below", () => {
    expect(revealRange(gap, "up")).toEqual({ from: 50, to: 69 });
  });

  it("▼ shows the twenty lines just below the change above", () => {
    expect(revealRange(gap, "down")).toEqual({ from: 10, to: 29 });
  });

  it("show opens the whole gap", () => {
    expect(revealRange(gap, "all")).toEqual({ from: 10, to: 69 });
  });

  it("never reaches outside a short gap", () => {
    expect(revealRange({ ...gap, oldTo: 15, hidden: 6 }, "up")).toEqual({ from: 10, to: 15 });
  });
});

describe("change navigation (#13)", () => {
  it("is on a change whose start sits in the anchor rows", () => {
    expect(changeSteps([4, 30, 80], 27, 60, 3)).toEqual({ prev: 0, next: 2 });
  });

  // Previous was off below the only change, and skipped the change just above the view.
  it("goes back to a change scrolled past", () => {
    expect(changeSteps([10], 50, 80, 3)).toEqual({ prev: 0, next: 1 });
    expect(changeSteps([4, 30, 80], 26, 60, 3)).toEqual({ prev: 0, next: 1 });
    expect(changeSteps([10, 50, 100], 70, 95, 3)).toEqual({ prev: 1, next: 2 });
  });

  it("counts the first change as current while the view is at the top and it is on screen", () => {
    expect(changeSteps([6, 30], 0, 25, 3)).toEqual({ prev: -1, next: 1 });
  });

  it("is before the first change when that one is still below the anchor", () => {
    expect(changeSteps([40, 90], 0, 25, 3)).toEqual({ prev: -1, next: 0 });
  });

  it("steps either side of the change a jump landed on", () => {
    expect(stepsAround(1)).toEqual({ prev: 0, next: 2 });
  });

  it("turns both buttons off for a single change", () => {
    expect(navState(1, stepsAround(0))).toEqual({ prev: false, next: false });
  });

  it("turns off previous on the first change and next on the last", () => {
    expect(navState(3, stepsAround(0))).toEqual({ prev: false, next: true });
    expect(navState(3, stepsAround(1))).toEqual({ prev: true, next: true });
    expect(navState(3, stepsAround(2))).toEqual({ prev: true, next: false });
  });

  it("offers next from above the first change and previous from below the last", () => {
    expect(navState(1, { prev: -1, next: 0 })).toEqual({ prev: false, next: true });
    expect(navState(1, { prev: 0, next: 1 })).toEqual({ prev: true, next: false });
  });

  it("offers nothing without changes, nor a step the changes no longer have", () => {
    expect(navState(0, { prev: -1, next: 0 })).toEqual({ prev: false, next: false });
    expect(navState(2, { prev: 4, next: 5 })).toEqual({ prev: false, next: false });
  });
});

// The whole file instead of "N lines hidden" (Diff and 3-way merge alike).
describe("the whole file", () => {
  it("keeps every unchanged line while it is shown, else the context lines", () => {
    expect(foldContext(true, 3)).toBe(Number.POSITIVE_INFINITY);
    expect(foldContext(false, 3)).toBe(3);
  });

  it("is missing lines while a gap between the hunks the backend sent is still closed", () => {
    const gap = (loaded: boolean): FoldEntry => ({
      kind: "gap",
      gap: { oldFrom: 1, oldTo: 9, newFrom: 1, hidden: 9, loaded, up: true, down: false, context: null },
    });
    expect(missingLines([gap(false)])).toBe(true);
    expect(missingLines([gap(true)])).toBe(false);
    expect(missingLines([])).toBe(false);
  });
});

describe("openingRow", () => {
  it("opens a diff at its first change, as Next Change puts it", () => {
    expect(openingRow([40, 90], 3)).toBe(37);
    expect(changeSteps([40, 90], 37, 60, 3)).toEqual(stepsAround(0));
  });

  it("stays at the top when the first change is there, or there is none", () => {
    expect(openingRow([1], 3)).toBe(0);
    expect(openingRow([], 3)).toBe(0);
  });
});
