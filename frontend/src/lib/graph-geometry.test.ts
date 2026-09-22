import { afterEach, describe, expect, it } from "vitest";
import {
  GRAPH,
  canvasPixelSize,
  gutterWidth,
  hitTest,
  centreRow,
  laneX,
  nextRow,
  nodeCentre,
  rowY,
  scrollRowIntoView,
  setLaneWidth,
  visibleRange,
  HEADER_ROWS,
  toCommitRow,
  toListRow,
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

describe("gutterWidth", () => {
  it("grows with the widest lane", () => {
    expect(gutterWidth(0, 1000)).toBe(GRAPH.leftPad + GRAPH.laneWidth);
    expect(gutterWidth(4, 1000)).toBe(GRAPH.leftPad + GRAPH.laneWidth * 5);
  });

  it("never eats more than a quarter of the panel", () => {
    expect(gutterWidth(50, 400)).toBeLessThanOrEqual(100);
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
  afterEach(() => setLaneWidth(14));

  it("widens the spacing between lanes", () => {
    setLaneWidth(24);
    expect(laneX(2)).toBe(GRAPH.leftPad + 48);
  });

  it("widens the gutter to match", () => {
    setLaneWidth(24);
    expect(gutterWidth(0, 1000)).toBe(GRAPH.leftPad + 24);
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
  it("draws a dot big enough to aim at with a mouse", () => {
    expect(GRAPH.nodeRadius * 2).toBeGreaterThanOrEqual(8);
  });

  it("draws lines thick enough to follow across a screen", () => {
    expect(GRAPH.lineWidth).toBeGreaterThanOrEqual(2);
  });

  // The line used to be drawn half a pixel off the dot, because an odd stroke width
  // needs that offset to stay crisp and the dots never got it. An even width needs no
  // offset at all, so both can sit on the same whole-pixel centre.
  it("uses an even line width, so nothing has to be nudged half a pixel", () => {
    expect(GRAPH.lineWidth % 2).toBe(0);
  });

  it("leaves a gap between a dot and the one below it", () => {
    expect(GRAPH.rowHeight).toBeGreaterThan(GRAPH.mergeRadius * 2 + 4);
  });

  it("leaves a gap between a dot and the one beside it", () => {
    expect(GRAPH.laneWidth).toBeGreaterThan(GRAPH.mergeRadius * 2);
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
