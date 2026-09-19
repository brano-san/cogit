import { describe, expect, it } from "vitest";
import {
  GRAPH,
  canvasPixelSize,
  gutterWidth,
  hitTest,
  laneX,
  rowY,
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
    // A buffer at the top of the list would otherwise index negatively.
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
    // Browsers can report an overscrolled position during momentum scrolling.
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
    // The canvas only covers the viewport, so rows are drawn in viewport space.
    expect(rowY(10, GRAPH.rowHeight * 10)).toBe(GRAPH.rowHeight / 2);
  });
});

describe("gutterWidth", () => {
  it("grows with the widest lane", () => {
    expect(gutterWidth(0, 1000)).toBe(GRAPH.leftPad + GRAPH.laneWidth);
    expect(gutterWidth(4, 1000)).toBe(GRAPH.leftPad + GRAPH.laneWidth * 5);
  });

  it("never eats more than a quarter of the panel", () => {
    // A repository with fifty parallel branches must not squeeze out the messages.
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
    // Without this the lines are blurred on a HiDPI display.
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
    // Clicking the Working Tree row must not select a commit that happens to sit at
    // the same index minus one.
    expect(toCommitRow(0)).toBeNull();
  });

  it("round-trips", () => {
    for (const row of [0, 1, 42, 9999]) {
      expect(toCommitRow(toListRow(row))).toBe(row);
    }
  });
});
