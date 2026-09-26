import { describe, expect, it } from "vitest";
import {
  BAND_WIDTH,
  NUM_WIDTH,
  SPLIT_MAX,
  SPLIT_MIN,
  bandLeft,
  draggedShare,
  ribbonPath,
  ribbonsNear,
  type Span,
} from "./diff-band";

function span(fromTop: number, fromBottom: number, toTop: number, toBottom: number): Span {
  return { fromTop, fromBottom, toTop, toBottom };
}

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

describe("ribbonsNear", () => {
  const all = [span(0, 2, 0, 2), span(100, 102, 300, 302), span(500, 501, 505, 506)];

  it("keeps a ribbon whose ends straddle the window", () => {
    expect(ribbonsNear(all, 150, 200)).toEqual([all[1]]);
  });

  it("keeps a ribbon that only reaches into the window from above", () => {
    expect(ribbonsNear(all, 101, 110)).toEqual([all[1]]);
  });

  it("drops everything outside the window", () => {
    expect(ribbonsNear(all, 1000, 1100)).toEqual([]);
  });

  it("keeps a ribbon sitting exactly on the edge", () => {
    expect(ribbonsNear([span(10, 12, 10, 12)], 12, 20)).toHaveLength(1);
    expect(ribbonsNear([span(10, 12, 10, 12)], 0, 10)).toHaveLength(1);
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
    expect(path).toContain("L 28 40");
  });

  it("spans a whole block rather than a single line", () => {
    const one = ribbonPath(span(0, 0, 0, 0), 18);
    const many = ribbonPath(span(0, 5, 0, 5), 18);

    expect(one).not.toEqual(many);
    expect(many).toContain("108");
  });
});
