import { describe, expect, it } from "vitest";
import { blockConnectors } from "./diff-band";
import { alignModel } from "./diff-aligned";
import { buildBlocks, type PaneRow } from "./diff-blocks";
import type { FoldEntry } from "./diff-fold";
import { mapLeftScrollToRight, mapRightScrollToLeft } from "./diff-sync";
import type { DiffRow } from "./ipc";

const ctx = (n: number, text = `c${n}`): FoldEntry => ({ kind: "row", row: { kind: "context", old: n, new: n, text }, block: 0 });
const del = (old: number, text: string): FoldEntry => ({ kind: "row", row: { kind: "delete", old, text, inline: [] } as DiffRow, block: 0 });
const ins = (n: number, text: string): FoldEntry => ({ kind: "row", row: { kind: "insert", new: n, text, inline: [] } as DiffRow, block: 0 });
const gap = (hidden: number): FoldEntry => ({
  kind: "gap",
  gap: { oldFrom: 1, oldTo: hidden, newFrom: 1, hidden, loaded: false, up: true, down: false, context: null },
});

const shape = (rows: PaneRow[]) => rows.map((r) => (r.kind === "line" ? r.text : r.kind === "gap" ? "gap" : "-"));

describe("alignModel", () => {
  it("opposes a pure insertion with as many filler rows as it has lines", () => {
    const m = alignModel(buildBlocks([ctx(1), ins(2, "a"), ins(3, "b"), ins(4, "c"), ctx(2)]));
    expect(shape(m.left)).toEqual(["c1", "-", "-", "-", "c2"]);
    expect(shape(m.right)).toEqual(["c1", "a", "b", "c", "c2"]);
  });

  it("opposes a pure deletion with filler on the right", () => {
    const m = alignModel(buildBlocks([ctx(1), del(2, "a"), del(3, "b"), ctx(4)]));
    expect(shape(m.left)).toEqual(["c1", "a", "b", "c4"]);
    expect(shape(m.right)).toEqual(["c1", "-", "-", "c4"]);
  });

  it("pairs a changed block by similarity and fills the rest", () => {
    const m = alignModel(
      buildBlocks([ctx(1), del(2, "if (not contains)"), del(3, "flush();"), ins(2, "// note"), ins(3, "if (heardAt.empty()) {"), ins(4, "flush();"), ctx(5)]),
    );
    expect(shape(m.left)).toEqual(["c1", "-", "if (not contains)", "flush();", "c5"]);
    expect(shape(m.right)).toEqual(["c1", "// note", "if (heardAt.empty()) {", "flush();", "c5"]);
  });

  it("keeps both panes the same length, every block on one row range of both", () => {
    const m = alignModel(buildBlocks([gap(5), ctx(6), del(7, "x"), ins(7, "yy"), ins(8, "zz"), ctx(8), ins(9, "q")]));
    expect(m.left).toHaveLength(m.right.length);
    for (const b of m.blocks) {
      expect(b.leftRow).toBe(b.rightRow);
      expect(b.leftRows).toBe(b.rightRows);
    }
    expect(m.left[0]!.kind).toBe("gap");
    expect(m.right[0]!.kind).toBe("gap");
    expect(m.blocks.reduce((n, b) => n + b.leftRows, 0)).toBe(m.left.length);
  });

  it("leaves nothing to scroll apart: the same scrollTop is the same row", () => {
    const m = alignModel(buildBlocks([ctx(1), del(2, "a"), ctx(3), ins(4, "b"), ins(5, "c"), ctx(6), ctx(7)]));
    const view = (scrollTop: number) => ({ scrollTop, viewport: 36 });
    for (const t of [0, 7, 18, 40]) {
      expect(mapLeftScrollToRight(m, view(t), view(0), 18)).toBe(Math.min(t, m.right.length * 18 - 36));
      expect(mapRightScrollToLeft(m, view(0), view(t), 18)).toBe(Math.min(t, m.left.length * 18 - 36));
    }
  });

  it("scrolls the panes 1:1 at every position, the last screenful included", () => {
    const rows = [ctx(1)];
    for (let i = 0; i < 30; i++) rows.push(i % 5 === 0 ? ins(2 + i, "x") : ctx(2 + i));
    const m = alignModel(buildBlocks(rows));
    const view = (scrollTop: number) => ({ scrollTop, viewport: 90 });
    const max = m.left.length * 18 - 90;
    for (let y = 0; y <= max; y++) {
      expect(mapLeftScrollToRight(m, view(y), view(0), 18), "at " + y).toBe(y);
      expect(mapRightScrollToLeft(m, view(0), view(y), 18), "at " + y).toBe(y);
    }
  });

  it("draws a straight band, level at both edges, with one button anchor per side", () => {
    const m = alignModel(buildBlocks([ctx(1), ins(2, "a"), ins(3, "b"), ctx(4)]));
    const view = { scrollTop: 0, viewport: 200 };
    const [c] = blockConnectors(m, view, view, 18, 9);
    expect(c!.left).toEqual([18, 54]);
    expect(c!.right).toEqual([18, 54]);
    expect(c!.anchor.left).toBe(36);
    expect(c!.anchor.right).toBe(36);
  });

  it("does not change the compact model", () => {
    const base = buildBlocks([ctx(1), ins(2, "a"), ctx(3)]);
    const before = base.left.length;
    alignModel(base);
    expect(base.left).toHaveLength(before);
    expect(base.blocks[1]!.leftRows).toBe(0);
  });
});
