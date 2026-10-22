import { afterEach, describe, expect, it } from "vitest";
import {
  GRAPH,
  LANE_WIDTH,
  canvasPixelSize,
  arrowStub,
  segmentCurve,
  svgPath,
  transition,
  type Curve,
  textX,
  hitTest,
  centreRow,
  laneX,
  nextRow,
  nodeCentre,
  nodeSquare,
  nodeFill,
  rowStarts,
  striped,
  rowY,
  scrollRowIntoView,
  setLaneWidth,
  visibleRange,
  MAX_SCROLL_PX,
  scrollScale,
  rescaledScrollTop,
  wheelStep,
  toVirtual,
  fromVirtual,
  HEADER_ROWS,
  listRowAt,
  clickedCommit,
  headNode,
  keyTarget,
  toCommitRow,
  toListRow,
  workingTreeLane,
  throughWorkingTree,
} from "./graph-geometry";

describe("visibleRange", () => {
  const rowHeight = 22;

  it("starts at the top when nothing is scrolled", () => {
    const { start } = visibleRange(0, 220, rowHeight, 1000, 0);
    expect(start).toBe(0);
  });

  it("covers the whole viewport", () => {
    const { start, end } = visibleRange(0, 220, rowHeight, 1000, 0);
    expect(end - start).toBeGreaterThanOrEqual(10);
  });

  it("follows the scroll position", () => {
    const { start } = visibleRange(rowHeight * 40, 220, rowHeight, 1000, 0);
    expect(start).toBe(40);
  });

  it("never starts before the first row", () => {
    const { start } = visibleRange(0, 220, rowHeight, 1000, 10);
    expect(start).toBe(0);
  });

  it("never ends past the last row", () => {
    const { end } = visibleRange(rowHeight * 995, 220, rowHeight, 1000, 10);
    expect(end).toBe(1000);
  });

  it("is empty for an empty list", () => {
    expect(visibleRange(0, 220, rowHeight, 0, 10)).toEqual({ start: 0, end: 0 });
  });

  it("extends by the buffer on both sides", () => {
    const plain = visibleRange(rowHeight * 40, 220, rowHeight, 1000, 0);
    const buffered = visibleRange(rowHeight * 40, 220, rowHeight, 1000, 5);
    expect(buffered.start).toBe(plain.start - 5);
    expect(buffered.end).toBe(plain.end + 5);
  });

  it("survives a scroll position beyond the content", () => {
    const { start, end } = visibleRange(rowHeight * 5000, 220, rowHeight, 100, 5);
    expect(start).toBeLessThanOrEqual(end);
    expect(end).toBeLessThanOrEqual(100);
  });
});

describe("laneX", () => {
  it("places the first lane at the left padding", () => {
    expect(laneX(0)).toBe(GRAPH.leftPad);
  });

  it("steps by the lane width", () => {
    expect(laneX(3)).toBe(GRAPH.leftPad + GRAPH.laneWidth * 3);
  });
});

describe("rowY", () => {
  it("puts a row at its centre relative to the scroll position", () => {
    expect(rowY(0, 0)).toBe(GRAPH.rowHeight / 2);
    expect(rowY(10, 0)).toBe(GRAPH.rowHeight * 10 + GRAPH.rowHeight / 2);
  });

  it("shifts with the scroll position", () => {
    expect(rowY(10, GRAPH.rowHeight * 10)).toBe(GRAPH.rowHeight / 2);
  });
});

// The message starts right of the last line of its own row, so it sits next
// to its node however wide the rows around it are (R-161).
describe("textX", () => {
  it("starts right of the one column a linear history uses", () => {
    expect(textX(1)).toBe(laneX(0) + GRAPH.laneWidth / 2 + GRAPH.textGap);
  });

  it("moves one column right for every column the row uses", () => {
    expect(textX(3) - textX(2)).toBe(GRAPH.laneWidth);
  });

  it("stops at the column cap, so a pathological row cannot push the text away", () => {
    expect(textX(GRAPH.maxColumns + 20)).toBe(textX(GRAPH.maxColumns));
  });

  it("treats a row with no columns as one", () => {
    expect(textX(0)).toBe(textX(1));
  });
});

describe("hitTest", () => {
  it("finds the row under the cursor", () => {
    const hit = hitTest(laneX(0), rowY(3, 0), 0, 100);
    expect(hit?.row).toBe(3);
  });

  it("finds the lane under the cursor", () => {
    const hit = hitTest(laneX(2), rowY(3, 0), 0, 100);
    expect(hit?.lane).toBe(2);
  });

  it("accounts for the scroll position", () => {
    const scroll = GRAPH.rowHeight * 40;
    const hit = hitTest(laneX(1), rowY(43, scroll), scroll, 100);
    expect(hit?.row).toBe(43);
  });

  it("returns nothing past the end of the list", () => {
    expect(hitTest(laneX(0), rowY(500, 0), 0, 100)).toBeNull();
  });

  it("returns nothing above the first row", () => {
    expect(hitTest(laneX(0), -10, 0, 100)).toBeNull();
  });
});

describe("canvasPixelSize", () => {
  it("scales the backing store by the device pixel ratio", () => {
    expect(canvasPixelSize(300, 200, 2)).toEqual({ width: 600, height: 400 });
  });

  it("rounds up so the canvas is never a fraction short", () => {
    expect(canvasPixelSize(300, 200, 1.5)).toEqual({ width: 450, height: 300 });
    expect(canvasPixelSize(101, 101, 1.25)).toEqual({ width: 127, height: 127 });
  });

  it("treats a missing ratio as 1", () => {
    expect(canvasPixelSize(300, 200, 0)).toEqual({ width: 300, height: 200 });
  });
});

describe("row offset for the Working Tree header", () => {
  it("shifts a commit row down past the header", () => {
    expect(toListRow(0)).toBe(HEADER_ROWS);
    expect(toListRow(7)).toBe(7 + HEADER_ROWS);
  });

  it("maps a list row back to its commit", () => {
    expect(toCommitRow(HEADER_ROWS)).toBe(0);
    expect(toCommitRow(HEADER_ROWS + 7)).toBe(7);
  });

  it("reports the header row itself as no commit", () => {
    expect(toCommitRow(0)).toBeNull();
  });

  it("round-trips", () => {
    for (const row of [0, 1, 42, 9999]) {
      expect(toCommitRow(toListRow(row))).toBe(row);
    }
  });
});

describe("setLaneWidth", () => {
  afterEach(() => setLaneWidth(16));

  it("widens the spacing between lanes", () => {
    setLaneWidth(24);
    expect(laneX(2)).toBe(GRAPH.leftPad + 48);
  });

  it("moves the text to match", () => {
    setLaneWidth(24);
    expect(textX(1)).toBe(GRAPH.leftPad + 24 / 2 + GRAPH.textGap);
  });

  it("refuses a width that would collapse the lanes onto each other", () => {
    setLaneWidth(0);
    expect(GRAPH.laneWidth).toBeGreaterThan(0);
  });

  it("rounds to whole pixels so lines stay crisp", () => {
    setLaneWidth(16.4);
    expect(GRAPH.laneWidth).toBe(16);
  });
});

describe("clickedCommit", () => {
  const oidAt = (row: number) => (row < 3 ? `c${row}` : undefined);

  it("selects the commit of a loaded row, and the working tree from the first row", () => {
    expect(clickedCommit(4, 3, oidAt)).toBe("c1");
    expect(clickedCommit(0, 3, oidAt)).toBeNull();
  });

  it("selects nothing on a rebase row or a row whose block has not arrived", () => {
    expect(clickedCommit(1, 3, oidAt)).toBeUndefined();
    expect(clickedCommit(9, 3, oidAt)).toBeUndefined();
  });
});

describe("keyTarget", () => {
  // Scrolled 10 000 rows away: the selected commit's block was evicted.
  const rows = {
    loadedIndexOf: (oid: string | null) => (oid === "near" ? 3 : null),
    indexOf: async (oid: string) => ({ near: 3, far: 600 })[oid] ?? null,
  };

  it("moves from a selected commit whose block is not loaded, not from the top", async () => {
    expect(await keyTarget(rows, "far", "ArrowDown", 1000, 20)).toBe(601);
    expect(await keyTarget(rows, "far", "PageUp", 1000, 20)).toBe(580);
  });

  it("moves from a loaded selection as before", async () => {
    expect(await keyTarget(rows, "near", "ArrowUp", 1000, 20)).toBe(2);
  });

  it("starts at the top with nothing selected or a commit this graph lacks", async () => {
    expect(await keyTarget(rows, null, "ArrowDown", 1000, 20)).toBe(0);
    expect(await keyTarget(rows, "gone", "ArrowDown", 1000, 20)).toBe(0);
  });
});

describe("nextRow", () => {
  it("moves down and up by one", () => {
    expect(nextRow(5, "ArrowDown", 100, 20)).toBe(6);
    expect(nextRow(5, "ArrowUp", 100, 20)).toBe(4);
  });

  it("stops at the ends instead of wrapping", () => {
    expect(nextRow(0, "ArrowUp", 100, 20)).toBe(0);
    expect(nextRow(99, "ArrowDown", 100, 20)).toBe(99);
  });

  it("moves by a page", () => {
    expect(nextRow(50, "PageDown", 100, 20)).toBe(70);
    expect(nextRow(50, "PageUp", 100, 20)).toBe(30);
  });

  it("clamps a page move to the ends", () => {
    expect(nextRow(5, "PageUp", 100, 20)).toBe(0);
    expect(nextRow(95, "PageDown", 100, 20)).toBe(99);
  });

  it("jumps to the first and last row", () => {
    expect(nextRow(50, "Home", 100, 20)).toBe(0);
    expect(nextRow(50, "End", 100, 20)).toBe(99);
  });

  it("starts from the top when nothing is selected", () => {
    expect(nextRow(null, "ArrowDown", 100, 20)).toBe(0);
    expect(nextRow(null, "ArrowUp", 100, 20)).toBe(0);
  });

  it("ignores a key that is not navigation", () => {
    expect(nextRow(5, "a", 100, 20)).toBeNull();
  });

  it("has nowhere to go in an empty list", () => {
    expect(nextRow(null, "ArrowDown", 0, 20)).toBeNull();
  });
});

describe("scrollRowIntoView", () => {
  const rowHeight = 22;

  it("leaves the scroll alone when the row is already visible", () => {
    expect(scrollRowIntoView(5, 0, 440, rowHeight)).toBeNull();
  });

  it("scrolls up to reach a row above the viewport", () => {
    expect(scrollRowIntoView(2, 220, 440, rowHeight)).toBe(2 * rowHeight);
  });

  it("scrolls down just far enough to reveal a row below it", () => {
    expect(scrollRowIntoView(30, 0, 440, rowHeight)).toBe(31 * rowHeight - 440);
  });

  it("never scrolls to a negative offset", () => {
    expect(scrollRowIntoView(0, 100, 440, rowHeight)).toBe(0);
  });
});

describe("centreRow", () => {
  const rowHeight = 22;

  it("puts the row in the middle of the viewport", () => {
    expect(centreRow(50, 440, rowHeight, 1000)).toBe(50 * rowHeight + rowHeight / 2 - 220);
  });

  it("does not scroll above the first row", () => {
    expect(centreRow(1, 440, rowHeight, 1000)).toBe(0);
  });

  it("does not scroll past the last row", () => {
    expect(centreRow(999, 440, rowHeight, 1000)).toBe(1000 * rowHeight - 440);
  });

  it("stays at zero when the whole list fits on screen", () => {
    expect(centreRow(3, 440, rowHeight, 5)).toBe(0);
  });

  it("survives a viewport that has not been measured yet", () => {
    expect(centreRow(10, 0, rowHeight, 1000)).toBeGreaterThanOrEqual(0);
  });
});

describe("HEAD's ring under the Working Tree row", () => {
  // A ticked origin/main two commits ahead: one line down lane 0, HEAD comes third.
  const lanes = [0, 0, 0, 0];
  const laneAt = (row: number) => lanes[row];

  it("is HEAD's own node when newer commits are drawn above it", () => {
    expect(headNode(2, laneAt, 1)).toEqual({ lane: 0, listRow: 3 });
  });

  it("counts the rebase rows above the first commit", () => {
    expect(headNode(0, () => 0, 4)).toEqual({ lane: 0, listRow: 4 });
  });

  it("is nowhere while HEAD is not in the graph or its row is not at hand", () => {
    expect(headNode(null, laneAt, 1)).toBeNull();
    expect(headNode(9, laneAt, 1)).toBeNull();
  });
});

describe("row offset with extra header rows", () => {
  it("shifts a commit past however many rows sit above it", () => {
    expect(toListRow(0, 1)).toBe(1);
    expect(toListRow(0, 5)).toBe(5);
    expect(toListRow(3, 5)).toBe(8);
  });

  it("maps a list row back to its commit", () => {
    expect(toCommitRow(8, 5)).toBe(3);
    expect(toCommitRow(5, 5)).toBe(0);
  });

  it("reports no commit for a row inside the header", () => {
    expect(toCommitRow(4, 5)).toBeNull();
    expect(toCommitRow(0, 5)).toBeNull();
  });

  it("still defaults to the single working-tree row", () => {
    expect(toListRow(0)).toBe(1);
    expect(toCommitRow(1)).toBe(0);
    expect(toCommitRow(0)).toBeNull();
  });
});

describe("how big the graph is drawn", () => {
  it("draws a ring big enough to aim at with a mouse", () => {
    expect(GRAPH.ringRadius * 2 + GRAPH.ringStroke).toBeGreaterThanOrEqual(8);
  });

  it("draws lines thick enough to follow across a screen", () => {
    expect(GRAPH.lineWidth).toBeGreaterThanOrEqual(2);
  });

  it("draws the main line a little thicker than the rest", () => {
    const extra = GRAPH.mainLineWidth - GRAPH.lineWidth;
    expect(extra).toBeGreaterThanOrEqual(0.5);
    expect(extra).toBeLessThanOrEqual(1);
  });

  it("leaves a gap between a ring and the one below it", () => {
    expect(GRAPH.rowHeight).toBeGreaterThan(GRAPH.ringRadius * 2 + GRAPH.ringStroke + 4);
    // 2.5 read as a speck beside the rings; a little under a ring's outer edge.
    expect(GRAPH.workingTreeRadius).toBeGreaterThanOrEqual(GRAPH.ringRadius);
    expect(GRAPH.workingTreeRadius).toBeLessThan(GRAPH.ringRadius + GRAPH.ringStroke / 2);
  });

  it("leaves a gap between a ring and the one beside it", () => {
    expect(GRAPH.laneWidth).toBeGreaterThan(GRAPH.ringRadius * 2 + GRAPH.ringStroke);
  });
});

describe("nodeCentre", () => {
  it("is where the line ends and where the dot is drawn, one and the same", () => {
    expect(nodeCentre(3, 7, 0)).toEqual({ x: laneX(3), y: rowY(7, 0) });
  });

  it("follows the scroll", () => {
    expect(nodeCentre(0, 10, GRAPH.rowHeight * 10).y).toBe(GRAPH.rowHeight / 2);
  });
});

describe("nodeSquare", () => {
  it("is centred where a ring would be, so lines meet it the same way", () => {
    const square = nodeSquare(3, 7, 0);
    const centre = nodeCentre(3, 7, 0);

    expect(square.x + square.size / 2).toBe(centre.x);
    expect(square.y + square.size / 2).toBe(centre.y);
  });

  it("is as wide as a ring", () => {
    expect(nodeSquare(0, 0, 0).size).toBe(GRAPH.ringRadius * 2);
  });
});

// A column change is one S inside one row: it leaves and arrives vertically, so
// consecutive rows join without a kink and nothing is ever drawn horizontally (R-161).
describe("segmentCurve", () => {
  const top = (row: number) => row * GRAPH.rowHeight;
  const centre = (row: number) => top(row) + GRAPH.rowHeight / 2;
  const seg = (from: number, to: number, span: "top" | "bottom" | "through") => ({ from, to, span, arrow: false });

  it("draws a lane passing through as the whole row, straight", () => {
    const curve = segmentCurve(seg(2, 2, "through"), 4, 0)!;
    expect([curve.x1, curve.y1, curve.x2, curve.y2]).toEqual([laneX(2), top(4), laneX(2), top(5)]);
  });

  it("meets the node at its centre from above and below", () => {
    const into = segmentCurve(seg(1, 1, "top"), 7, 0)!;
    const out = segmentCurve(seg(1, 1, "bottom"), 7, 0)!;
    expect({ x: into.x2, y: into.y2 }).toEqual(nodeCentre(1, 7, 0));
    expect({ x: out.x1, y: out.y1 }).toEqual(nodeCentre(1, 7, 0));
  });

  it("turns a branch out of its parent over one row: from the node to the next row's centre", () => {
    const fork = segmentCurve(seg(3, 1, "top"), 7, 0)!;
    expect({ x: fork.x1, y: fork.y1 }).toEqual(nodeCentre(1, 7, 0));
    expect([fork.x2, fork.y2]).toEqual([laneX(3), centre(6)]);
  });

  it("turns a merge into its second parent's column over one row", () => {
    const merge = segmentCurve(seg(1, 3, "bottom"), 7, 0)!;
    expect({ x: merge.x1, y: merge.y1 }).toEqual(nodeCentre(1, 7, 0));
    expect([merge.x2, merge.y2]).toEqual([laneX(3), centre(8)]);
  });

  it("leaves the half row a turn covers to the turn", () => {
    const merged = [seg(1, 3, "bottom")];
    expect(segmentCurve(seg(3, 3, "through"), 8, 0, { above: merged })).toMatchObject({ y1: centre(8), y2: top(9) });
    const forked = [seg(3, 1, "top")];
    expect(segmentCurve(seg(3, 3, "through"), 6, 0, { below: forked })).toMatchObject({ y1: top(6), y2: centre(6) });
    expect(segmentCurve(seg(3, 3, "top"), 8, 0, { above: merged })).toBeNull();
    expect(segmentCurve(seg(3, 3, "bottom"), 6, 0, { below: forked })).toBeNull();
  });

  it("joins two nodes in one curve when a line leaves one and enters the next in a third column", () => {
    const leaving = segmentCurve(seg(0, 1, "bottom"), 3, 0, { below: [seg(1, 2, "top")] })!;
    expect({ x: leaving.x2, y: leaving.y2 }).toEqual(nodeCentre(2, 4, 0));
    expect(segmentCurve(seg(1, 2, "top"), 4, 0, { above: [seg(0, 1, "bottom")] })).toBeNull();
  });

  it("follows the scroll", () => {
    const curve = segmentCurve(seg(0, 0, "through"), 10, GRAPH.rowHeight * 10)!;
    expect(curve.y1).toBe(0);
  });
});

describe("transition", () => {
  const node = nodeCentre(2, 5, 0);
  const mirrorY = (curve: Curve): Curve => ({
    x1: curve.x1, y1: 2 * node.y - curve.y1, cx1: curve.cx1, cy1: 2 * node.y - curve.cy1,
    cx2: curve.cx2, cy2: 2 * node.y - curve.cy2, x2: curve.x2, y2: 2 * node.y - curve.y2,
  });
  const mirrorX = (curve: Curve): Curve => ({
    x1: 2 * node.x - curve.x1, y1: curve.y1, cx1: 2 * node.x - curve.cx1, cy1: curve.cy1,
    cx2: 2 * node.x - curve.cx2, cy2: curve.cy2, x2: 2 * node.x - curve.x2, y2: curve.y2,
  });

  it("is the same shape for a branch leaving upwards and a merge going down: mirrored", () => {
    for (const lane of [0, 3, 4]) {
      expect(mirrorY(transition(node, laneX(lane), "up"))).toEqual(transition(node, laneX(lane), "down"));
    }
  });

  it("is the same shape to the left and to the right: mirrored", () => {
    const right = transition(node, laneX(3), "down");
    const left = transition(node, laneX(1), "down");
    expect(mirrorX(right)).toEqual(left);
  });

  it("leaves the node on a slant and meets the column upright", () => {
    const curve = transition(node, laneX(3), "down");
    expect(curve.cx1).not.toBe(curve.x1);
    expect(curve.cy1).not.toBe(curve.y1);
    expect(curve.cx2).toBe(curve.x2);
  });
});

describe("edges at a node", () => {
  it("all end in the node's centre", () => {
    const row = 6;
    const centre = nodeCentre(1, row, 0);
    const ends = [
      segmentCurve({ from: 1, to: 1, span: "top" }, row, 0)!,
      segmentCurve({ from: 3, to: 1, span: "top" }, row, 0)!,
      segmentCurve({ from: 1, to: 1, span: "bottom" }, row, 0)!,
      segmentCurve({ from: 1, to: 0, span: "bottom" }, row, 0)!,
      segmentCurve({ from: 1, to: 2, span: "bottom" }, row, 0)!,
    ].map((curve) => [{ x: curve.x1, y: curve.y1 }, { x: curve.x2, y: curve.y2 }]);
    for (const pair of ends) expect(pair).toContainEqual(centre);
  });
});

describe("mini graphs, as drawn", () => {
  type Seg = { from: number; to: number; span: "top" | "bottom" | "through"; arrow: boolean };
  const s = (from: number, to: number, span: Seg["span"]): Seg => ({ from, to, span, arrow: false });
  const draw = (rows: Seg[][]) =>
    rows.map((segments, row) =>
      segments.map((segment) => {
        const curve = segmentCurve(segment, row, 0, { above: rows[row - 1], below: rows[row + 1] });
        return curve ? svgPath(curve) : null;
      }),
    );

  it("a commit forked from above and merged from below in the next column", () => {
    // tip (1) ─ its parent C (0) is a merge whose second parent Q (1) is the next row.
    expect(draw([
      [s(1, 1, "bottom")],
      [s(1, 0, "top"), s(0, 0, "bottom"), s(0, 1, "bottom")],
      [s(0, 0, "through"), s(1, 1, "top"), s(1, 1, "bottom")],
      [s(0, 0, "top"), s(1, 0, "top")],
    ])).toMatchSnapshot();
  });

  it("branches leaving to the left and to the right", () => {
    expect(draw([
      [s(0, 0, "bottom")],
      [s(0, 0, "through"), s(2, 2, "bottom")],
      [s(0, 1, "top"), s(2, 1, "top"), s(1, 1, "bottom")],
    ])).toMatchSnapshot();
  });

  it("a merge across two columns", () => {
    expect(draw([
      [s(1, 1, "through"), s(0, 0, "bottom"), s(0, 2, "bottom")],
      [s(0, 0, "through"), s(1, 1, "through"), s(2, 2, "top"), s(2, 2, "bottom")],
    ])).toMatchSnapshot();
  });

  it("an octopus", () => {
    expect(draw([
      [s(0, 0, "bottom"), s(0, 1, "bottom"), s(0, 2, "bottom"), s(0, 3, "bottom")],
      [s(0, 0, "through"), s(1, 1, "through"), s(2, 2, "through"), s(3, 3, "top")],
    ])).toMatchSnapshot();
  });
});

describe("arrowStub", () => {
  it("points down from the node and stays inside its own row", () => {
    const stub = arrowStub({ from: 1, to: 1 }, 4, 0);
    const centre = nodeCentre(1, 4, 0);
    expect(stub.x1).toBe(centre.x);
    expect(stub.x2).toBe(centre.x);
    expect(stub.y1).toBeGreaterThan(centre.y);
    expect(stub.y2).toBeGreaterThan(stub.y1);
    expect(stub.y2).toBeLessThanOrEqual(5 * GRAPH.rowHeight);
  });

  it("leans right for a later parent, off the first parent's line, and stays short", () => {
    const stub = arrowStub({ from: 1, to: 2 }, 4, 0);
    const centre = nodeCentre(1, 4, 0);
    expect(stub.x1).toBeGreaterThan(centre.x);
    expect(stub.x2).toBeGreaterThan(stub.x1);
    expect(stub.y2).toBeGreaterThan(stub.y1);
    expect(stub.x2).toBeLessThan(laneX(2) - GRAPH.laneWidth / 2);
    expect(stub.y2).toBeLessThanOrEqual(5 * GRAPH.rowHeight);
  });
});

describe("row stripes", () => {
  it("stripes every second row, starting with the one under the Working Tree row", () => {
    expect([0, 1, 2, 3].map((row) => striped(row))).toEqual([false, true, false, true]);
  });

  it("fills a ring with the panel, and with the stripe on a striped row", () => {
    expect(nodeFill(2, [], null)).toEqual(["--surface-panel"]);
    expect(nodeFill(3, [], null)).toEqual(["--surface-panel", "--row-stripe"]);
  });

  it("lets hover and then selection cover the stripe, as they do on the row", () => {
    expect(nodeFill(3, [], 3).at(-1)).toBe("--state-hover");
    expect(nodeFill(3, [3], 3).at(-1)).toBe("--state-selected");
    expect(nodeFill(3, [5], 4)).toEqual(["--surface-panel", "--row-stripe"]);
  });

  // The other end of a comparison has the selected background too; its ring showed a hole.
  it("fills the ring of every row drawn selected", () => {
    expect(nodeFill(3, [5, 3], null).at(-1)).toBe("--state-selected");
    expect(nodeFill(5, [5, 3], null).at(-1)).toBe("--state-selected");
  });

  // Repositories (#41): a repository draws the submodules open under it, so the next one
  // starts that many rows further down; a row the list does not draw takes none.
  it("starts each item of a list after every row the ones above it draw", () => {
    expect(rowStarts([1, 3, 0, 2, 1])).toEqual([0, 1, 4, 4, 6]);
    expect(rowStarts([])).toEqual([]);
    expect(rowStarts([1, 3, 0, 2, 1]).map((row) => striped(row))).toEqual([false, true, false, false, false]);
  });

  it("stripes nothing, rings included, once the banding is switched off", () => {
    expect([0, 1, 2, 3].map((row) => striped(row, false))).toEqual([false, false, false, false]);
    expect(nodeFill(3, [], null, false)).toEqual(["--surface-panel"]);
    expect(nodeFill(3, [3], null, false)).toEqual(["--surface-panel", "--state-selected"]);
  });
});

describe("Working Tree row above a HEAD that is not the first commit", () => {
  it("sits right above HEAD and keeps the commits above it in place", () => {
    const at = 3;
    expect([0, 1, 2].map((row) => toListRow(row, 1, at))).toEqual([0, 1, 2]);
    expect(toListRow(3, 1, at)).toBe(4);
    expect(toCommitRow(3, 1, at)).toBeNull();
    expect(clickedCommit(3, 1, (row) => `c${row}`, at)).toBeNull();
    expect(clickedCommit(4, 1, (row) => `c${row}`, at)).toBe("c3");
    for (const row of [0, 2, 3, 50]) expect(toCommitRow(toListRow(row, 1, at), 1, at)).toBe(row);
    expect(headNode(3, () => 0, 1, at)).toEqual({ lane: 0, listRow: 4 });
  });

  it("takes HEAD's lane when free, else the first lane right of it no line crosses", () => {
    expect(workingTreeLane(1, [{ from: 0, span: "through" }])).toBe(1);
    expect(workingTreeLane(0, [{ from: 0, span: "top" }, { from: 1, span: "through" }])).toBe(2);
    expect(workingTreeLane(0, [{ from: 0, span: "top" }, { from: 2, span: "bottom" }])).toBe(1);
    expect(workingTreeLane(2, [{ from: 2, span: "top" }])).toBe(3);
    // The arrow of a cut link above HEAD is a stub inside its row, not a line through the Working Tree.
    expect(workingTreeLane(0, [{ from: 0, span: "top", arrow: true }])).toBe(0);
  });
});

describe("listRowAt", () => {
  it("follows the live scroll offset, not the row the element sits in", () => {
    expect(listRowAt(30, 0, 24, 100)).toBe(1);
    expect(listRowAt(30, 720, 24, 100)).toBe(31);
  });

  it("is nothing above the viewport, past the last row or with no row height", () => {
    expect(listRowAt(-1, 0, 24, 10)).toBeNull();
    expect(listRowAt(240, 0, 24, 10)).toBeNull();
    expect(listRowAt(10, 0, 0, 10)).toBeNull();
  });
});

describe("a list taller than the layout limit", () => {
  const viewport = 600;

  it("is not scaled while it fits", () => {
    const k = scrollScale(100_000 * 24, viewport);
    expect(k).toBe(1);
    expect(toVirtual(12_345, k)).toBe(12_345);
    expect(fromVirtual(12_345, k)).toBe(12_345);
  });

  it("keeps the real height under the cap and still reaches the last row", () => {
    const rows = 1_500_000;
    const rowHeight = 28;
    const virtualPx = rows * rowHeight;
    const k = scrollScale(virtualPx, viewport);

    expect(k).toBeGreaterThan(1);
    const furthest = virtualPx - viewport;
    const realFurthest = fromVirtual(furthest, k);
    expect(realFurthest + viewport).toBeLessThanOrEqual(MAX_SCROLL_PX);
    expect(toVirtual(realFurthest, k)).toBeCloseTo(furthest, 3);
    expect(visibleRange(furthest, viewport, rowHeight, rows, 0).end).toBe(rows);
    expect(fromVirtual(centreRow(rows - 1, viewport, rowHeight, rows), k)).toBeLessThanOrEqual(
      realFurthest,
    );
  });

  // The history grows while it is walked: the element keeps its scrollTop, the scale changes.
  it("keeps the same row at the top when the history grows and the scale with it", () => {
    const rowHeight = 24;
    const before = scrollScale(400_000 * rowHeight, viewport);
    const after = scrollScale(900_000 * rowHeight, viewport);
    expect(after).toBeGreaterThan(before);

    const real = fromVirtual(200_000 * rowHeight, before);
    const moved = rescaledScrollTop(real, before, after);

    // Without it the element offset `real` stood for `after / before` times as far.
    expect(Math.floor(toVirtual(real, after) / rowHeight)).toBeGreaterThan(200_000);
    expect(Math.floor(toVirtual(moved, after) / rowHeight)).toBe(200_000);
  });

  it("keeps the offset when a list stops being scaled, too", () => {
    expect(rescaledScrollTop(1000, 2, 1)).toBe(2000);
    expect(rescaledScrollTop(1000, 1, 1)).toBe(1000);
  });

  // Past the cap the element moves k times as far per notch; the wheel is taken over.
  it("steps the wheel by pixels, lines or pages as an unscaled list would", () => {
    expect(wheelStep(100, 0, 24, 600)).toBe(100);
    expect(wheelStep(3, 1, 24, 600)).toBe(72);
    expect(wheelStep(-1, 2, 24, 600)).toBe(-600);
  });

  it("round-trips a position", () => {
    const k = scrollScale(1_500_000 * 24, viewport);
    expect(toVirtual(fromVirtual(9_876_543, k), k)).toBeCloseTo(9_876_543, 3);
  });
});

describe("the gap around a local commit", () => {
  it("is there, and stays clear of the next lane at the narrowest width", () => {
    expect(GRAPH.localGap).toBeGreaterThan(0);
    expect(GRAPH.ringRadius + GRAPH.ringStroke / 2 + GRAPH.localGap).toBeLessThan(LANE_WIDTH.min / 2);
  });
});

// The Working Tree row above HEAD further down: HEAD's lines are drawn on across it. A turn
// from the commit above ends at that row's centre, and the line drawn from its top beside
// the turn's last half made two lines there.
describe("a line drawn on across the Working Tree row", () => {
  const row = 5;
  const top = row * GRAPH.rowHeight;

  it("starts at the row's top", () => {
    expect(throughWorkingTree(row, 0, [{ from: 0, to: 0, span: "through" }], 0)).toBe(top);
  });

  it("starts at its centre where a turn from the row above comes into the same column", () => {
    const above = [{ from: 1, to: 0, span: "bottom" as const }];
    expect(throughWorkingTree(row, 0, above, 0)).toBe(top + GRAPH.rowHeight / 2);
  });
});
