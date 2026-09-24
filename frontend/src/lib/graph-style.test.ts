import { describe, expect, it } from "vitest";
import { GRAPH } from "$lib/graph-geometry";
import {
  BRANCH_SLOTS,
  DIM_ALPHA,
  FOCUS_LINE_WIDTH,
  LAYERS,
  branchSlot,
  laneAt,
  nodeStroke,
  rowPaint,
  segmentStroke,
} from "$lib/graph-style";
import type { GraphOverlay, GraphRow, Segment } from "$lib/ipc";

const segment = (primary: boolean, color = 3): Segment => ({
  from: 0,
  to: 0,
  span: "through",
  primary,
  color,
  arrow: false,
});

const row = (primary: boolean): GraphRow => ({
  row: 0,
  lane: 0,
  color: 2,
  kind: "normal",
  primary,
  width: 1,
  segments: [],
  links: [],
});

const plain = { colouredLanes: false };

describe("branchSlot", () => {
  it("is fixed by the name alone, so a branch keeps its colour between runs", () => {
    // Pinned: a change here recolours every user's branches.
    expect(["main", "master", "develop", "feature/login", "hotfix"].map(branchSlot)).toEqual([0, 3, 2, 3, 7]);
  });

  it("spreads names over every slot", () => {
    const used = new Set(Array.from({ length: 200 }, (_, i) => branchSlot(`feature/${i}`)));
    expect(used.size).toBe(BRANCH_SLOTS);
  });
});

describe("rowPaint", () => {
  const overlay: GraphOverlay = {
    start: 128,
    total: 300,
    nodeLanes: [4, 5],
    nodeStyles: [0, 3],
    segmentFirst: [0, 1, 3],
    segmentLanes: [4, 5, 6],
    segmentStyles: [0, 3, 1],
    folds: [],
  };

  it("cuts one row's segments out of the window", () => {
    expect(rowPaint(overlay, 129)).toEqual({
      nodeLane: 5,
      nodeStyle: 3,
      segmentLanes: [5, 6],
      segmentStyles: [3, 1],
    });
  });

  it("has nothing for a row outside the window", () => {
    expect(rowPaint(overlay, 127)).toBeUndefined();
    expect(rowPaint(overlay, 130)).toBeUndefined();
  });
});

describe("strokes", () => {
  const painted = { nodeLane: 1, nodeStyle: 5, segmentLanes: [1], segmentStyles: [5] };

  it("are the monochrome graph without paint: the main line bright and wide, the rest grey", () => {
    expect(segmentStroke(segment(true), 0, undefined, plain)).toMatchObject({
      token: "--graph-main",
      width: GRAPH.mainLineWidth,
    });
    expect(segmentStroke(segment(false), 0, undefined, plain)).toMatchObject({
      token: "--graph-line",
      width: GRAPH.lineWidth,
    });
  });

  it("take a ticked branch's colour, drawn over the grey and under the main line", () => {
    const coloured = segmentStroke(segment(false), 0, painted, plain);
    expect(coloured.token).toBe("--graph-branch-5");
    expect(coloured.layer).toBeGreaterThan(segmentStroke(segment(false), 0, undefined, plain).layer);
    expect(coloured.layer).toBeLessThan(segmentStroke(segment(true), 0, undefined, plain).layer);
  });

  it("fall back to the lane colour under Coloured branch lines where no branch has one", () => {
    expect(segmentStroke(segment(false, 3), 0, undefined, { colouredLanes: true }).token).toBe("--c-lane-4");
    expect(segmentStroke(segment(false, 3), 0, painted, { colouredLanes: true }).token).toBe("--graph-branch-5");
  });

  it("keep a ring's width whatever its colour", () => {
    expect(nodeStroke(row(false), painted, plain)).toMatchObject({
      token: "--graph-branch-5",
      width: GRAPH.ringStroke,
    });
    expect(nodeStroke(row(true), undefined, plain)).toMatchObject({
      token: "--graph-main",
      width: GRAPH.ringStroke,
    });
  });
});

describe("the branch of the chosen commit", () => {
  const paint = { nodeLane: 7, nodeStyle: 0, segmentLanes: [7, 2], segmentStyles: [0, 0] };
  const focus = { colouredLanes: false, focusLane: 7 };

  it("is drawn on top of everything and wider than the main line", () => {
    const focused = segmentStroke(segment(false), 0, paint, focus);
    expect(focused).toMatchObject({ token: "--graph-focus", width: FOCUS_LINE_WIDTH });
    expect(focused.layer).toBe(LAYERS - 1);
    expect(FOCUS_LINE_WIDTH).toBeGreaterThan(GRAPH.mainLineWidth);
    expect(segmentStroke(segment(false), 1, paint, focus)).toMatchObject({ token: "--graph-line" });
  });

  it("keeps its own colour when it is a ticked branch or the main line", () => {
    const coloured = { ...paint, segmentStyles: [3, 0] };
    expect(segmentStroke(segment(false), 0, coloured, focus).token).toBe("--graph-branch-3");
    expect(segmentStroke(segment(true), 0, paint, focus).token).toBe("--graph-main");
  });
});

describe("laneAt", () => {
  const layout: GraphRow = {
    ...row(false),
    lane: 1,
    segments: [
      { from: 0, to: 0, span: "through", primary: true, color: 0, arrow: false },
      { from: 2, to: 1, span: "top", primary: false, color: 4, arrow: false },
      { from: 1, to: 3, span: "bottom", primary: false, color: 5, arrow: false },
    ],
  };
  const paint = { nodeLane: 9, nodeStyle: 0, segmentLanes: [0, 4, 5], segmentStyles: [0, 0, 0] };

  it("finds the line under a click by its column and half of the row", () => {
    expect(laneAt(layout, paint, 0, true)).toBe(0);
    expect(laneAt(layout, paint, 2, true)).toBe(4);
    expect(laneAt(layout, paint, 3, false)).toBe(5);
    expect(laneAt(layout, paint, 3, true)).toBeNull();
  });

  it("is the node's own lane on the ring's column, and nothing without paint", () => {
    expect(laneAt(layout, paint, 1, false)).toBe(9);
    expect(laneAt(layout, undefined, 0, true)).toBeNull();
  });
});

describe("ancestry", () => {
  it("draws what lies outside it faint and under everything else", () => {
    const dimmed = { nodeLane: 1, nodeStyle: 0x10 | 2, segmentLanes: [1], segmentStyles: [0x10] };
    const line = segmentStroke(segment(true), 0, dimmed, plain);
    expect(line).toMatchObject({ alpha: DIM_ALPHA, layer: 0, token: "--graph-main" });
    expect(nodeStroke(row(false), dimmed, plain)).toMatchObject({ alpha: DIM_ALPHA, token: "--graph-branch-2" });
    expect(segmentStroke(segment(false), 0, undefined, plain).alpha).toBe(1);
  });
});
