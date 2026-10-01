import { describe, expect, it } from "vitest";
import { BAND_WIDTH } from "./diff-band";
import {
  LINE_HEIGHT,
  alignedPads,
  bandConnectors,
  contentHeight,
  edgesOfBlocks,
  mapScroll,
  panesOf,
  scrollToBlock,
  type BlockRows,
} from "./solver-geometry";

const H = LINE_HEIGHT;

describe("layouts", () => {
  it("names the panes each layout shows", () => {
    expect(panesOf("oursResult")).toEqual({ ours: true, theirs: false, below: false });
    expect(panesOf("all")).toEqual({ ours: true, theirs: true, below: false });
    expect(panesOf("resultTheirs")).toEqual({ ours: false, theirs: true, below: false });
    expect(panesOf("resultBelow")).toEqual({ ours: true, theirs: true, below: true });
  });
});

describe("aligned padding", () => {
  const rows = [
    { ours: 1, result: 1, theirs: 3 },
    { ours: 0, result: 2, theirs: 1 },
  ];

  it("pads every pane to the tallest of the panes shown, at the end of the hunk", () => {
    expect(alignedPads(rows, { ours: true, result: true, theirs: true })).toEqual({
      ours: [2, 2],
      result: [2, 0],
      theirs: [0, 1],
    });
  });

  it("a pane that is not shown does not make the others taller", () => {
    expect(alignedPads(rows, { ours: true, result: true, theirs: false })).toEqual({
      ours: [0, 2],
      result: [0, 0],
      theirs: [0, 0],
    });
  });

  it("with the Result below, only Ours and Theirs line up with each other", () => {
    expect(alignedPads(rows, { ours: true, result: false, theirs: true })).toEqual({
      ours: [2, 1],
      result: [0, 0],
      theirs: [0, 0],
    });
  });
});

describe("where a pane's hunks sit", () => {
  const blocks: BlockRows[] = [
    { start: 2, count: 3 },
    { start: 5, count: 0 },
    { start: 9, count: 1 },
  ];

  it("counts a line at its row, and the padding of the hunks above", () => {
    expect(edgesOfBlocks(blocks, [0, 0, 0])).toEqual([
      { top: 2 * H, bottom: 5 * H },
      { top: 5 * H, bottom: 5 * H },
      { top: 9 * H, bottom: 10 * H },
    ]);
    expect(edgesOfBlocks(blocks, [1, 0, 2])).toEqual([
      { top: 2 * H, bottom: 6 * H },
      { top: 6 * H, bottom: 6 * H },
      { top: 10 * H, bottom: 13 * H },
    ]);
  });

  it("the whole content is its lines and its padding", () => {
    expect(contentHeight(12, [1, 0, 2])).toBe(15 * H);
  });
});

describe("keeping two panes level", () => {
  const ours = [
    { top: 2 * H, bottom: 3 * H },
    { top: 10 * H, bottom: 10 * H },
  ];
  const result = [
    { top: 2 * H, bottom: 5 * H },
    { top: 12 * H, bottom: 14 * H },
  ];

  it("is the identity where the panes are padded to each other", () => {
    expect(mapScroll(ours, ours, 7 * H, 100 * H, 100 * H, 10 * H)).toBe(7 * H);
  });

  it("stretches a hunk onto the other pane's taller one and moves on past it", () => {
    expect(mapScroll(ours, result, 0, 40 * H, 40 * H, 10 * H)).toBe(0);
    expect(mapScroll(ours, result, 2 * H, 40 * H, 40 * H, 10 * H)).toBe(2 * H);
    expect(mapScroll(ours, result, 2.5 * H, 40 * H, 40 * H, 10 * H)).toBe(2 * H + 1.5 * H);
    expect(mapScroll(ours, result, 3 * H, 40 * H, 40 * H, 10 * H)).toBe(5 * H);
    expect(mapScroll(ours, result, 8 * H, 40 * H, 40 * H, 10 * H)).toBe(10 * H);
  });

  it("a pane at its bottom puts the other at its own", () => {
    expect(mapScroll(ours, result, 30 * H, 40 * H, 44 * H, 10 * H)).toBe(34 * H);
  });

  it("the target never leaves its scroll range", () => {
    expect(mapScroll(ours, result, 31 * H, 45 * H, 16 * H, 10 * H)).toBe(6 * H);
  });
});

describe("the bands between two panes", () => {
  const left = [
    { top: 2 * H, bottom: 3 * H },
    { top: 40 * H, bottom: 41 * H },
  ];
  const right = [
    { top: 2 * H, bottom: 4 * H },
    { top: 40 * H, bottom: 40 * H },
  ];

  it("draws a ribbon only for the hunks near the viewport", () => {
    const made = bandConnectors(left, right, 0, 0, 10 * H, [1, 3], 9);
    expect(made.map((c) => c.id)).toEqual([1]);
    expect(made[0]?.left).toEqual([2 * H, 3 * H]);
    expect(made[0]?.right).toEqual([2 * H, 4 * H]);
    expect(made[0]?.path.startsWith(`M 0 ${2 * H}`)).toBe(true);
  });

  it("follows each pane's own scroll", () => {
    const made = bandConnectors(left, right, 1 * H, 0, 10 * H, [1, 3], 9);
    expect(made[0]?.left).toEqual([1 * H, 2 * H]);
    expect(made[0]?.right).toEqual([2 * H, 4 * H]);
  });

  it("puts a button on its ribbon, and none where the ribbon is off screen", () => {
    const [made] = bandConnectors(left, right, 0, 0, 10 * H, [1, 3], 9);
    expect(made?.anchor.left).not.toBeNull();
    expect(made?.anchor.right).not.toBeNull();
    const far = bandConnectors(left, right, 38 * H, 38 * H, 10 * H, [1, 3], 9);
    expect(far.map((c) => c.id)).toEqual([3]);
    expect(far[0]?.right).toEqual([2 * H, 2 * H]);
  });

  it("stays inside the gutter", () => {
    expect(BAND_WIDTH).toBeGreaterThan(0);
  });
});

describe("scrolling a hunk into view", () => {
  it("lands the hunk a few rows below the top of the pane", () => {
    expect(scrollToBlock({ top: 50 * H, bottom: 52 * H }, 20 * H, 400 * H)).toBe(47 * H);
  });

  it("stays at the top for a hunk near it, and at the bottom for one near the end", () => {
    expect(scrollToBlock({ top: 1 * H, bottom: 2 * H }, 20 * H, 400 * H)).toBe(0);
    expect(scrollToBlock({ top: 395 * H, bottom: 396 * H }, 20 * H, 400 * H)).toBe(380 * H);
  });
});
