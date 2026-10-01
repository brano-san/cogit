import { describe, expect, it } from "vitest";
import {
  ACTION_LEFT_X,
  ACTION_RIGHT_X,
  BAND_WIDTH,
  curveFraction,
  NUM_WIDTH,
  SPLIT_MAX,
  SPLIT_MIN,
  bandLeft,
  draggedShare,
  blockConnectors,
  edgesOf,
  pathOf,
  type PaneView,
} from "./diff-band";
import { alignModel } from "./diff-aligned";
import { buildBlocks } from "./diff-blocks";
import type { FoldEntry, Gap } from "./diff-fold";
import type { DiffRow } from "./ipc";

interface Span {
  fromTop: number;
  fromBottom: number;
  toTop: number;
  toBottom: number;
}
function span(fromTop: number, fromBottom: number, toTop: number, toBottom: number): Span {
  return { fromTop, fromBottom, toTop, toBottom };
}
/** Rows to pixels, as `blockConnectors` does it. */
const edgesPx = (c: Span, h: number) => [c.fromTop * h, (c.fromBottom + 1) * h, c.toTop * h, (c.toBottom + 1) * h] as const;
const ribbonPath = (c: Span, h: number) => pathOf(...edgesPx(c, h));
const ribbonEdges = (c: Span, h: number) => edgesOf(...edgesPx(c, h));

describe("bandLeft", () => {
  it("centres the band in the row at an even split", () => {
    expect(bandLeft(1000, 0.5, 80)).toBe((1000 - BAND_WIDTH) / 2);
  });

  it("gives the left code column its share of the code width", () => {
    expect(bandLeft(1000, 0.3, 80)).toBeCloseTo(80 + 0.3 * (1000 - 2 * 80 - BAND_WIDTH));
  });

  it("never lets the band slide over the line numbers", () => {
    expect(bandLeft(0, 0.5, 80)).toBe(80);
    expect(bandLeft(50, 0.8, 80)).toBe(80);
    expect(bandLeft(0, 0.5, 0)).toBe(NUM_WIDTH);
  });
});

describe("the split between the columns", () => {
  it("moves by the dragged pixels as a share of the code width", () => {
    const code = 1000 - 2 * 80 - BAND_WIDTH;
    expect(draggedShare(0.5, code / 10, 1000, 80)).toBeCloseTo(0.6);
  });

  it("keeps each column at least a fifth of the code width", () => {
    expect(draggedShare(0.5, -5000, 1000, 80)).toBe(SPLIT_MIN);
    expect(draggedShare(0.5, 5000, 1000, 80)).toBe(SPLIT_MAX);
    expect(SPLIT_MIN).toBeCloseTo(1 - SPLIT_MAX);
  });

  it("stays put in a row with no room for code", () => {
    expect(draggedShare(0.4, 30, 100, 80)).toBe(0.4);
  });
});

describe("ribbonPath", () => {
  it("closes the outline, so the shape can be filled", () => {
    expect(ribbonPath(span(0, 0, 4, 4), 18).endsWith(" Z")).toBe(true);
  });

  it("measures the edges in rows, not pixels of its own", () => {
    const path = ribbonPath(span(1, 1, 3, 3), 10);

    // Left edge from row 1 to the bottom of row 1; right edge rows 3 to 4.
    expect(path.startsWith("M 0 10 ")).toBe(true);
    expect(path).toContain("L 42 40");
  });

  it("leaves both boundaries horizontally, controls at the midpoint", () => {
    const path = ribbonPath(span(1, 2, 5, 6), 10);

    expect(path).toContain("M 0 10 C 21 10 21 50 42 50");
    expect(path).toContain("C 21 70 21 30 0 30");
  });

  it("collapses a zero-height side to a point", () => {
    // Pure insert: no rows on the left (bottom is one above top).
    const path = ribbonPath(span(3, 2, 3, 5), 10);

    expect(path.startsWith("M 0 30 C 21 30 21 30 42 30")).toBe(true);
    expect(path).toContain("0 30 Z");
  });

  it("strokes only the two curves", () => {
    const edges = ribbonEdges(span(1, 2, 5, 6), 10);

    expect(edges).toBe("M 0 10.5 C 21 10.5 21 50.5 42 50.5 M 42 69.5 C 21 69.5 21 29.5 0 29.5");
    expect(edges).not.toContain("L");
  });

  it("puts a level edge on the middle of one pixel row, as a straight line", () => {
    expect(ribbonEdges(span(1, 2, 1, 2), 10)).toBe("M 0 10.5 L 42 10.5 M 42 29.5 L 0 29.5");
  });

  it("keeps the tip of a wedge on the row boundary", () => {
    expect(ribbonEdges(span(2, 1, 2, 3), 10)).toBe(
      "M 0 20 C 21 20 21 20.5 42 20.5 M 42 39.5 C 21 39.5 21 20 0 20",
    );
  });

  it("spans a whole block rather than a single line", () => {
    const one = ribbonPath(span(0, 0, 0, 0), 18);
    const many = ribbonPath(span(0, 5, 0, 5), 18);

    expect(one).not.toEqual(many);
    expect(many).toContain("108");
  });
});

describe("block connectors", () => {
  const ctx = (o: number, n: number): DiffRow => ({ kind: "context", old: o, new: n, text: "c" });
  const del = (o: number, extra = {}): DiffRow => ({ kind: "delete", old: o, text: "d", inline: [], ...extra });
  const ins = (n: number, extra = {}): DiffRow => ({ kind: "insert", new: n, text: "i", inline: [], ...extra });
  const model = (...r: DiffRow[]) => buildBlocks(r.map((row): FoldEntry => ({ kind: "row", row, block: 0 })));
  const view = (scrollTop = 0, viewport = 200, top = 0): PaneView => ({ scrollTop, viewport, top });
  // c c [d d] [i i i] c : left rows 0,1 | 2,3 | 4 ; right rows 0,1 | 2,3,4 | 5
  const m = model(ctx(1, 1), ctx(2, 2), del(3), del(4), ins(3), ins(4), ins(5), ctx(5, 6));

  it("spans the block's own rows on each side, in gutter pixels", () => {
    const [c] = blockConnectors(m, view(), view(), 18);
    expect(c).toMatchObject({ block: 1, kind: "changed", left: [36, 72], right: [36, 90] });
  });

  it("follows each pane's own scroll and header offset", () => {
    const [c] = blockConnectors(m, view(10, 200, 20), view(0, 200, 0), 18);
    expect(c!.left).toEqual([46, 82]);
    expect(c!.right).toEqual([36, 90]);
  });

  it("collapses the empty side of an added or removed block to a point", () => {
    const a = model(ctx(1, 1), ins(2), ctx(2, 3));
    const [c] = blockConnectors(a, view(), view(), 18);
    expect(c).toMatchObject({ kind: "added", left: [18, 18], right: [18, 36] });
    const r = model(ctx(1, 1), del(2), ctx(3, 2));
    expect(blockConnectors(r, view(), view(), 18)[0]).toMatchObject({ kind: "removed", left: [18, 36], right: [18, 18] });
  });

  it("culls blocks fully outside the viewport", () => {
    expect(blockConnectors(m, view(500), view(500), 18)).toEqual([]);
    expect(blockConnectors(m, view(0, 30), view(0, 30), 18)).toHaveLength(0);
    expect(blockConnectors(m, view(0, 40), view(0, 40), 18)).toHaveLength(1);
  });

  it("leaves horizontally at both edges of the band", () => {
    const [c] = blockConnectors(m, view(), view(), 18);
    const bend = BAND_WIDTH / 2;
    expect(c!.path.startsWith(`M 0 36 C ${bend} 36 ${bend} 36 ${BAND_WIDTH} 36`)).toBe(true);
    expect(c!.path.endsWith("Z")).toBe(true);
    expect(c!.path).toContain(`${BAND_WIDTH} 90`);
  });

  it("puts » in the left part of the gutter and × in the right, each on the band at its x", () => {
    const [c] = blockConnectors(m, view(), view(), 18);
    // The band spans [36, 72] at the left column and [36, 90] at the right: a button sits at
    // the middle of what the band is where the button is, so the right one is lower.
    expect(c!.anchor.left!).toBeGreaterThanOrEqual(36);
    expect(c!.anchor.left!).toBeLessThan(c!.anchor.right!);
    expect(c!.anchor.right!).toBeLessThanOrEqual(90);
    expect(ACTION_LEFT_X).toBeLessThan(BAND_WIDTH / 2);
    expect(ACTION_RIGHT_X).toBeGreaterThan(BAND_WIDTH / 2);
  });

  it("puts both buttons at one height on a level band", () => {
    const a = model(ctx(1, 1), ins(2), ins(3), ctx(2, 4));
    const v = view(0, 200);
    const [c] = blockConnectors(alignModel(a), v, v, 18, 9);
    expect(c!.anchor).toEqual({ left: 36, right: 36 });
  });

  it("clamps the anchor into the visible part when the block is partly off screen", () => {
    const [c] = blockConnectors(m, view(54, 60), view(54, 60), 18);
    for (const y of [c!.anchor.left, c!.anchor.right]) {
      expect(y!).toBeGreaterThanOrEqual(0);
      expect(y!).toBeLessThanOrEqual(60);
    }
    const [d] = blockConnectors(m, view(0, 50), view(0, 50), 18);
    expect(d!.anchor.right!).toBeLessThanOrEqual(50);
  });

  // R-628 (b): × hung at the bottom of the gutter, below all content. The old anchor was the
  // mean of the band's two edges, clamped: a band slanting from the top of the screen to far
  // below it averaged to a point nowhere on it and was pushed to the bottom edge.
  it("draws no button where the band is not on screen, instead of pinning it to the edge", () => {
    const m2 = model(ctx(1, 1), del(2), ctx(3, 2));
    // Band from left [18, 36] (scrolled to -400: far above) to right [-82+18.. ] on screen.
    const [c] = blockConnectors(m2, view(436, 300), view(-82, 300), 18, 9);
    expect(c).toBeDefined();
    expect(c!.anchor.left).toBeNull();
    expect(c!.anchor.right).not.toBeNull();
    // Entirely below the screen on one side and above on the other: crosses, buttons on it.
    const [d] = blockConnectors(m2, view(-300, 200), view(400, 200), 18, 9);
    for (const y of [d?.anchor.left, d?.anchor.right]) if (y != null) expect(y).toBeLessThanOrEqual(200 - 9);
  });

  // R-628 (a): a block with no lines on one side converges at that block's own insertion
  // point of that pane, with the pane's own scroll, a fold counted as the one row it is.
  it("converges an added block at its own insertion row, with a fold above counted as one row", () => {
    const gap: FoldEntry = {
      kind: "gap",
      gap: { oldFrom: 1, oldTo: 400, newFrom: 1, hidden: 400, loaded: true, up: false, down: true, context: null } as Gap,
    };
    const rows = [
      gap,
      ...[ctx(401, 401), ctx(402, 402), ins(403), ins(404), ins(405), ctx(403, 406)].map((row): FoldEntry => ({ kind: "row", row, block: 0 })),
    ];
    const fm = buildBlocks(rows);
    const [c] = blockConnectors(fm, view(0, 600), view(0, 600), 18);
    // left: gap, c, c | insertion before row 3. right: gap, c, c, i, i, i
    expect(c!.left).toEqual([54, 54]);
    expect(c!.right).toEqual([54, 108]);
    // Each pane's own scroll moves its own side only.
    const [d] = blockConnectors(fm, view(18, 600), view(36, 600), 18);
    expect(d!.left).toEqual([36, 36]);
    expect(d!.right).toEqual([18, 72]);
  });

  it("evaluates the S-curve the way the path draws it", () => {
    expect(curveFraction(0)).toBeCloseTo(0, 6);
    expect(curveFraction(BAND_WIDTH)).toBeCloseTo(1, 6);
    expect(curveFraction(BAND_WIDTH / 2)).toBeCloseTo(0.5, 6);
    expect(curveFraction(ACTION_LEFT_X)).toBeCloseTo(1 - curveFraction(ACTION_RIGHT_X), 6);
  });

  it("joins a moved block's two ends however far apart they sit", () => {
    const mv = model(del(1, { moved: true, moveId: 3 }), ctx(2, 1), ctx(3, 2), ctx(4, 3), ins(4, { moved: true, moveId: 3 }));
    const cs = blockConnectors(mv, view(), view(), 18);
    expect(cs).toHaveLength(1);
    expect(cs[0]).toMatchObject({ kind: "moved", left: [0, 18], right: [54, 72], moveId: 3 });
  });

  it("draws one connector per block with equal blocks left out", () => {
    expect(blockConnectors(m, view(), view(), 18).map((c) => c.block)).toEqual([1]);
  });

  it("finds the visible blocks of a huge file without walking it all", () => {
    const rows: DiffRow[] = [];
    for (let i = 1; i <= 100_000; i++) rows.push(i % 10 === 0 ? del(i) : ctx(i, i - Math.floor(i / 10)));
    const big = model(...rows.slice(0, 60_000));
    const t0 = performance.now();
    for (let i = 0; i < 100; i++) blockConnectors(big, view(18 * 30_000, 600), view(18 * 30_000, 600), 18);
    expect(performance.now() - t0).toBeLessThan(100);
  });
});
