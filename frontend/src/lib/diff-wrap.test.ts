import { describe, expect, it, vi } from "vitest";
import { blockConnectors } from "./diff-band";
import { alignModel } from "./diff-aligned";
import { buildBlocks, type BlockModel } from "./diff-blocks";
import type { FoldEntry } from "./diff-fold";
import { mapLeftScrollToRight, mapRightScrollToLeft, wheelScroll } from "./diff-sync";
import {
  WrapEstimate,
  advanceTable,
  anchoredTop,
  pairCounts,
  rowCounts,
  uniformRows,
  visibleRows,
  wrapCount,
  wrappedRows,
  type PaneGeometry,
} from "./diff-wrap";
import type { DiffRow } from "./ipc";
import { budget } from "./perf-budget";
import { rowMarks } from "./scroll-marks";

const RH = 18;
/** A monospace font 10 px wide, CJK twice that. */
const mono = (code: number) => (code >= 0x2e80 ? 20 : 10);
const estimate = () => new WrapEstimate(mono);

const ctx = (n: number, text = `c${n}`): FoldEntry => ({ kind: "row", row: { kind: "context", old: n, new: n, text }, block: 0 });
const del = (old: number, text: string): FoldEntry => ({ kind: "row", row: { kind: "delete", old, text, inline: [] } as DiffRow, block: 0 });
const ins = (n: number, text: string): FoldEntry => ({ kind: "row", row: { kind: "insert", new: n, text, inline: [] } as DiffRow, block: 0 });
const gap = (hidden: number): FoldEntry => ({
  kind: "gap",
  gap: { oldFrom: 1, oldTo: hidden, newFrom: 1, hidden, loaded: false, up: true, down: false, context: null },
});
const view = (scrollTop = 0, viewport = 100) => ({ scrollTop, viewport });

/** The geometry the view builds: each side's visual rows at its own width; Aligned, pairs. */
function geometry(model: BlockModel, widths: [number, number], aligned: boolean): PaneGeometry {
  const est = estimate();
  const left = rowCounts(model.left, widths[0], est, new Map());
  const right = rowCounts(model.right, widths[1], est, new Map());
  if (aligned) {
    const both = wrappedRows(pairCounts(left, right), RH);
    return { left: both, right: both };
  }
  return { left: wrappedRows(left, RH), right: wrappedRows(right, RH) };
}

describe("wrapCount", () => {
  it("fills a row while the characters fit and starts the next one", () => {
    expect(wrapCount("", 100, mono)).toBe(1);
    expect(wrapCount("x".repeat(10), 100, mono)).toBe(1);
    expect(wrapCount("x".repeat(11), 100, mono)).toBe(2);
    expect(wrapCount("x".repeat(25), 100, mono)).toBe(3);
  });

  // `white-space: break-spaces`: a space takes room like any character, also at a row's end.
  it("wraps spaces like any other character", () => {
    expect(wrapCount("abcd efghi ", 50, mono)).toBe(3);
    expect(wrapCount("    ", 30, mono)).toBe(2);
  });

  it("runs a tab to the next stop from the start of its visual row", () => {
    // Tab stops every 4 spaces = 40 px: "ab" + tab ends at 40, "xyz" makes 70.
    expect(wrapCount("ab\txyz", 70, mono, 4)).toBe(1);
    expect(wrapCount("ab\txyzw", 70, mono, 4)).toBe(2);
    // A tab past the edge starts the next row whole.
    expect(wrapCount("abcde\t", 60, mono, 4)).toBe(2);
  });

  it("counts a wide character as two columns and never splits it", () => {
    expect(wrapCount("中文中文中", 100, mono)).toBe(1);
    expect(wrapCount("a中文中文中", 100, mono)).toBe(2);
  });

  it("keeps a character wider than the column on a row of its own", () => {
    expect(wrapCount("中中", 15, mono)).toBe(2);
  });

  it("does not wrap past a width it cannot use", () => {
    expect(wrapCount("x".repeat(500), 0, mono)).toBe(1);
  });
});

describe("advanceTable and WrapEstimate", () => {
  it("measures each character once", () => {
    const measure = vi.fn((text: string) => text.length * 7);
    const advance = advanceTable(measure);
    for (let i = 0; i < 3; i++) expect(advance(0x61)).toBe(7);
    expect(advance(0x1f600)).toBe(14);
    expect(advance(0x1f600)).toBe(14);
    expect(measure).toHaveBeenCalledTimes(2);
  });

  it("agrees with wrapCount and keeps a line that fits at one row without walking it", () => {
    const est = estimate();
    for (const text of ["", "short", "x".repeat(250), "a\tb\tc".repeat(30), "中".repeat(33)]) {
      for (const width of [70, 100, 333]) expect(est.rows(text, width)).toBe(wrapCount(text, width, mono));
    }
  });
});

describe("row geometry", () => {
  it("is the old arithmetic unwrapped", () => {
    const rows = uniformRows(5, RH);
    expect([rows.top(0), rows.top(3), rows.top(5), rows.total, rows.height(2), rows.rowAt(40)]).toEqual([0, 54, 90, 90, 18, 2]);
    expect(rows.rowAt(1000)).toBe(55);
  });

  it("stacks wrapped rows and finds the row under a pixel", () => {
    const rows = wrappedRows([1, 3, 0, 2], RH);
    expect([0, 1, 2, 3, 4].map((i) => rows.top(i))).toEqual([0, 18, 72, 72, 108]);
    expect([0, 1, 2, 3].map((i) => rows.height(i))).toEqual([18, 54, 0, 36]);
    expect(rows.total).toBe(108);
    expect(rows.rowAt(0)).toBe(0);
    expect(rows.rowAt(17)).toBe(0);
    expect(rows.rowAt(18)).toBe(1);
    expect(rows.rowAt(71)).toBe(1);
    // The empty row (Aligned filler of no height) is skipped for the one starting there.
    expect(rows.rowAt(72)).toBe(3);
    // Past the end the rows go on at the base height, like `floor(y / h)`.
    expect(rows.rowAt(108)).toBe(4);
    expect(rows.rowAt(108 + 40)).toBe(6);
    expect(rows.rowAt(-1)).toBe(-1);
  });

  it("draws the rows around the viewport, by pixel, not by index", () => {
    const rows = wrappedRows([1, 10, 1, 1, 1, 1, 1, 1], RH);
    // y 0..100 is row 0 and part of row 1; one row of buffer past each edge.
    expect(visibleRows(rows, 0, 100, 1)).toEqual({ start: 0, end: 3 });
    expect(visibleRows(rows, 200, 100, 0)).toEqual({ start: 2, end: 8 });
    expect(visibleRows(uniformRows(0, RH), 0, 100, 3)).toEqual({ start: 0, end: 0 });
  });

  // The line number is drawn once per line, at the top of its row; wrapped, the next number is
  // as many visual rows lower as the line before it takes, never on a continuation row.
  it("puts each line number on the first visual row of its line", () => {
    const model = buildBlocks([ctx(1, "x".repeat(25)), ctx(2, "y"), ctx(3, "z".repeat(11))]);
    const counts = rowCounts(model.unified, 100, estimate(), new Map());
    expect([...counts]).toEqual([3, 1, 2]);
    const rows = wrappedRows(counts, RH);
    expect([0, 1, 2].map((i) => rows.top(i) / RH)).toEqual([0, 3, 4]);
  });

  it("counts a fold as one row and filler as none, and a measured row as measured", () => {
    const model = alignModel(buildBlocks([gap(40), ctx(41, "x".repeat(30)), ins(42, "y".repeat(30))]));
    expect(model.left.map((row) => row.kind)).toEqual(["gap", "line", "filler"]);
    expect([...rowCounts(model.left, 100, estimate(), new Map())]).toEqual([1, 3, 0]);
    expect([...rowCounts(model.left, 100, estimate(), new Map([[1, 4]]))]).toEqual([1, 4, 0]);
  });

  it("keeps the top row where it was when rows above it change height", () => {
    const before = wrappedRows([1, 1, 1, 1], RH);
    const after = wrappedRows([3, 1, 1, 1], RH);
    // 5 px into row 2 before, 5 px into row 2 after.
    expect(anchoredTop(before, after, 2 * RH + 5)).toBe(4 * RH + 5);
    expect(anchoredTop(before, after, 0)).toBe(0);
    // Past the rows there is nothing to keep.
    expect(anchoredTop(before, after, 500)).toBe(500);
  });
});

describe("aligned rows of different wrapped heights", () => {
  // Left line wraps to 3 rows, its pair on the right to 1: the pair is 3 rows on both sides.
  const model = alignModel(
    buildBlocks([ctx(1), del(2, "a".repeat(25)), ins(2, "b"), ctx(3, "c".repeat(15)), ctx(4)]),
  );

  it("makes a pair as tall as its taller side on both sides", () => {
    const rows = geometry(model, [100, 100], true);
    expect(rows.left).toBe(rows.right);
    expect([0, 1, 2, 3, 4].map((i) => rows.left.top(i) / RH)).toEqual([0, 1, 4, 6, 7]);
  });

  it("joins the panes by a straight band of the pair's real height", () => {
    const rows = geometry(model, [100, 60], true);
    const [band] = blockConnectors(model, view(), view(), rows);
    expect(band!.left).toEqual([RH, 4 * RH]);
    expect(band!.right).toEqual(band!.left);
  });

  it("scrolls both panes as one", () => {
    const rows = geometry(model, [100, 60], true);
    for (const y of [0, 20, 50, 70]) {
      expect(mapLeftScrollToRight(model, view(y, 40), view(0, 40), rows)).toBe(y);
      expect(mapRightScrollToLeft(model, view(0, 40), view(y, 40), rows)).toBe(y);
    }
  });
});

describe("compact panes of different wrapped heights", () => {
  // Left: c1 [a×25 = 3 rows] c3; right: c1 [b, b] c3. The changed block is 54 px left, 36 right.
  const model = buildBlocks([ctx(1), del(2, "a".repeat(25)), ins(2, "b"), ins(3, "b"), ctx(3), ctx(4), ctx(5), ctx(6)]);
  const rows = geometry(model, [100, 100], false);

  it("draws a connector from each side's own wrapped edges", () => {
    const [band] = blockConnectors(model, view(), view(), rows);
    expect(band!.left).toEqual([RH, 4 * RH]);
    expect(band!.right).toEqual([RH, 3 * RH]);
  });

  it("maps a scroll through a block by its pixel heights, both ways", () => {
    // Halfway into the left block (27 px of 54) is halfway into the right one (18 of 36).
    expect(mapLeftScrollToRight(model, view(RH + 27, 20), view(0, 20), rows)).toBe(RH + 18);
    expect(mapRightScrollToLeft(model, view(0, 20), view(RH + 18, 20), rows)).toBe(RH + 27);
    // Below the block the panes run 1:1, offset by the difference of its heights.
    expect(mapLeftScrollToRight(model, view(5 * RH, 20), view(0, 20), rows)).toBe(4 * RH);
  });

  it("keeps each pane to its own wrapped length on the wheel", () => {
    const next = wheelScroll(model, view(0, 20), view(0, 20), 10_000, rows);
    expect(next).toEqual({ left: 8 * RH - 20, right: 7 * RH - 20 });
  });
});

describe("scroll marks over wrapped rows", () => {
  it("stand where their rows are, not where their index would be", () => {
    // A line wrapped to 10 rows, then a change: by index the change is the second row, by pixels the eleventh.
    const rows = wrappedRows([10, 1, 1], RH);
    const [mark] = rowMarks(["", "add", ""], rows, 120);
    expect(mark!.top).toBe(100);
    expect(mark!.height).toBe(10);
    const [plain] = rowMarks(["", "add", ""], RH, 120);
    expect(plain!.top).toBe(RH);
  });
});

describe("wrapping a 10 000-line diff", () => {
  // Every tenth line changed, a third of the lines long enough to wrap.
  const entries: FoldEntry[] = [];
  for (let i = 1; i <= 10_000; i++) {
    const text = `${i % 3 === 0 ? "long ".repeat(40) : "short"} line ${i}`;
    entries.push(i % 10 === 0 ? del(i, text) : ctx(i, text));
    if (i % 10 === 0) entries.push(ins(i, `${text} changed`));
  }
  const model = alignModel(buildBlocks(entries));

  it("builds the geometry of every row within a frame's budget, and a width change too", () => {
    const est = estimate();
    let t0 = performance.now();
    const counts = (width: number) =>
      wrappedRows(pairCounts(rowCounts(model.left, width, est, new Map()), rowCounts(model.right, width, est, new Map())), RH);
    const rows = counts(400);
    const cold = performance.now() - t0;
    t0 = performance.now();
    counts(380);
    const rewrap = performance.now() - t0;
    expect(rows.count).toBe(model.left.length);
    expect(rows.total).toBeGreaterThan(model.left.length * RH);
    expect(cold).toBeLessThan(budget(60));
    expect(rewrap).toBeLessThan(budget(40));
  });

  it("finds the visible window, its connectors and the other pane's scroll in well under a frame", () => {
    const rows = geometry(model, [400, 400], true);
    const t0 = performance.now();
    for (let frame = 0; frame < 100; frame++) {
      const y = (rows.left.total * frame) / 100;
      visibleRows(rows.left, y, 800, 12);
      blockConnectors(model, view(y, 800), view(y, 800), rows, 9);
      mapLeftScrollToRight(model, view(y, 800), view(y, 800), rows);
    }
    expect(performance.now() - t0).toBeLessThan(budget(20));
  });
});
