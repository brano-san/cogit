import { describe, expect, it } from "vitest";
import type { PaneView } from "./diff-band";
import { buildBlocks } from "./diff-blocks";
import type { FoldEntry, Gap } from "./diff-fold";
import { mapLeftScrollToRight, mapRightScrollToLeft, wheelScroll } from "./diff-sync";
import type { DiffRow } from "./ipc";

const RH = 18;
const ctx = (o: number, n: number): DiffRow => ({ kind: "context", old: o, new: n, text: "c" });
const del = (o: number, extra = {}): DiffRow => ({ kind: "delete", old: o, text: "d", inline: [], ...extra });
const ins = (n: number, extra = {}): DiffRow => ({ kind: "insert", new: n, text: "i", inline: [], ...extra });
const entries = (r: DiffRow[]): FoldEntry[] => r.map((row) => ({ kind: "row", row, block: 0 }));
const view = (scrollTop = 0, viewport = 10, top = 0): PaneView => ({ scrollTop, viewport, top });

// left rows:  c0 c1 [d2 d3] c4 c5 c6 c7         (8 rows)
// right rows: c0 c1 [i2 i3 i4 i5] c6 c7 c8 c9   (10 rows)
const rows = [
  ctx(1, 1), ctx(2, 2), del(3), del(4), ins(3), ins(4), ins(5), ins(6),
  ctx(5, 7), ctx(6, 8), ctx(7, 9), ctx(8, 10),
];
const m = buildBlocks(entries(rows));
const toR = (y: number) => mapLeftScrollToRight(m, view(y), view(), RH);
const toL = (y: number) => mapRightScrollToLeft(m, view(), view(y), RH);
const MAX_L = 8 * RH - 10;
const MAX_R = 10 * RH - 10;

describe("synchronized scroll", () => {
  it("is 1:1 in equal zones", () => {
    expect(toR(18)).toBe(18);
    expect(toR(5 * RH)).toBe(7 * RH);
    expect(toL(7 * RH)).toBe(5 * RH);
  });

  it("is proportional to the block's two lengths inside a changed block", () => {
    // left block is 2 rows, right block 4: a left row is two right rows
    expect(toR(2 * RH + 9)).toBe(2 * RH + 18);
    expect(toL(2 * RH + 18)).toBe(2 * RH + 9);
  });

  it("agrees at the block edges from both sides", () => {
    expect(toR(4 * RH)).toBe(6 * RH);
    expect(toL(6 * RH)).toBe(4 * RH);
  });

  it("round-trips to the pixel in equal zones and in blocks that grow", () => {
    for (let y = 0; y <= MAX_L - 10; y++) expect(toL(toR(y))).toBe(y);
  });

  it("round-trips within a couple of pixels when the block shrinks", () => {
    for (let y = 0; y <= 150; y++) expect(Math.abs(toR(toL(y)) - y)).toBeLessThanOrEqual(2);
  });

  it("never goes backwards while the source scrolls forward (no jitter)", () => {
    let last = -1;
    for (let y = 0; y < 8 * RH; y++) {
      const t = toR(y);
      expect(t).toBeGreaterThanOrEqual(last);
      last = t;
    }
  });

  it("a pure insert holds the short side still; the next scroll jumps over it", () => {
    const only = buildBlocks(entries([ctx(1, 1), ins(2), ins(3), ctx(2, 4), ...Array.from({ length: 12 }, (_, i) => ctx(3 + i, 5 + i))]));
    for (const y of [RH, RH + 9, 2 * RH, 3 * RH - 1]) {
      expect(mapRightScrollToLeft(only, view(0), view(y), RH)).toBe(RH);
    }
    expect(mapLeftScrollToRight(only, view(RH), view(0), RH)).toBe(3 * RH);
  });

  it("accounts for each pane's header offset", () => {
    expect(mapLeftScrollToRight(m, view(18, 10, 30), view(0, 10, 10), RH)).toBe(0);
    // left content y 70 is 34 into a 36 px block -> right 36 + 68, plus the right header
    expect(mapLeftScrollToRight(m, view(100, 10, 30), view(0, 10, 10), RH)).toBe(114);
  });

  it("pins to the bottom of panes of different content height", () => {
    expect(mapLeftScrollToRight(m, view(0), view(50), RH)).toBe(0);
    expect(mapLeftScrollToRight(m, view(MAX_L), view(0), RH)).toBe(MAX_R);
    expect(mapRightScrollToLeft(m, view(0), view(MAX_R), RH)).toBe(MAX_L);
  });

  // R-628 (a): a pane that reached its bottom pinned the other to ITS bottom at once, so the
  // last pixel of scroll jumped the other pane by the whole difference in length and every
  // connector on the last screen turned into a long diagonal. The pin now eases in over the
  // last screenful of the source, and the jump is gone.
  it("reaches the bottom of the other pane without a jump", () => {
    // Twenty one-row insertions: the right pane is 20 rows longer by the end.
    const r: DiffRow[] = [];
    let o = 1;
    let n = 1;
    for (let k = 0; k < 20; k++) {
      r.push(ctx(o++, n++), ctx(o++, n++), ins(n++));
    }
    for (let k = 0; k < 10; k++) r.push(ctx(o++, n++));
    const tall = buildBlocks(entries(r));
    const v = (y: number) => view(y, 90);
    const maxL = (o - 1) * RH - 90;
    const maxR = (n - 1) * RH - 90;
    let last = mapLeftScrollToRight(tall, v(0), v(0), RH);
    for (let y = 1; y <= maxL; y++) {
      const t = mapLeftScrollToRight(tall, v(y), v(0), RH);
      expect(t - last, "at " + y).toBeLessThanOrEqual(2 * RH);
      expect(t).toBeGreaterThanOrEqual(last);
      last = t;
    }
    expect(last).toBe(maxR);
    let back = mapRightScrollToLeft(tall, v(0), v(0), RH);
    for (let y = 1; y <= maxR; y++) {
      const t = mapRightScrollToLeft(tall, v(0), v(y), RH);
      expect(Math.abs(t - back), "at " + y).toBeLessThanOrEqual(2 * RH);
      back = t;
    }
    expect(back).toBe(maxL);
  });

  it("clamps a result past either end instead of overshooting", () => {
    expect(mapLeftScrollToRight(m, view(8 * RH), view(0), RH)).toBe(MAX_R);
  });

  it("counts a collapsed zone as one row on both sides and follows an expansion", () => {
    const gap: FoldEntry = {
      kind: "gap",
      gap: { oldFrom: 2, oldTo: 51, newFrom: 2, hidden: 50, loaded: true, up: true, down: true, context: null } as Gap,
    };
    const tail = [del(52), ins(52), ins(53), ctx(53, 54), ...Array.from({ length: 12 }, (_, i) => ctx(54 + i, 55 + i))];
    const folded = buildBlocks([...entries([ctx(1, 1)]), gap, ...entries(tail)]);
    // left: c0 gap d2 c3 | right: c0 gap i2 i3 c4
    expect(mapLeftScrollToRight(folded, view(2 * RH), view(), RH)).toBe(2 * RH);
    expect(mapLeftScrollToRight(folded, view(3 * RH), view(), RH)).toBe(4 * RH);
    const ctxRun = Array.from({ length: 50 }, (_, i) => ctx(2 + i, 2 + i));
    const opened = buildBlocks(entries([ctx(1, 1), ...ctxRun, ...tail]));
    // left: 51 rows of context then d51 c52 | right: ... i51 i52 c53
    expect(mapLeftScrollToRight(opened, view(51 * RH), view(), RH)).toBe(51 * RH);
    expect(mapLeftScrollToRight(opened, view(52 * RH), view(), RH)).toBe(53 * RH);
  });

  it("moves both panes by the same wheel delta over the gutter, clamped per pane", () => {
    expect(wheelScroll(m, view(40), view(70), 30, RH)).toEqual({ left: 70, right: 100 });
    expect(wheelScroll(m, view(40), view(70), -100, RH)).toEqual({ left: 0, right: 0 });
    expect(wheelScroll(m, view(MAX_L - 5), view(100), 500, RH)).toEqual({ left: MAX_L, right: MAX_R });
  });

  it("syncs a moved block as a removal at one end and an insertion at the other", () => {
    const mv = buildBlocks(entries([del(1, { moved: true, moveId: 1 }), ctx(2, 1), ctx(3, 2), ins(3, { moved: true, moveId: 1 })]));
    expect(mapLeftScrollToRight(mv, view(RH), view(), RH)).toBe(0);
  });
});
