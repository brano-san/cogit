import { describe, expect, it } from "vitest";
import { expandedContext, searchRows, stepHit } from "./diff-rows";

describe("expandedContext", () => {
  it("adds a screenful of lines", () => {
    expect(expandedContext(3, false)).toBe(23);
  });

  it("opens the whole file when asked", () => {
    expect(expandedContext(3, true)).toBeGreaterThan(9000);
  });

  it("keeps growing on repeated clicks", () => {
    expect(expandedContext(expandedContext(3, false), false)).toBe(43);
  });
});

describe("searching inside a diff", () => {
  it("finds nothing for an empty query", () => {
    expect(searchRows([["alpha", "beta"]], "")).toEqual([]);
    expect(searchRows([["alpha", "beta"]], "   ")).toEqual([]);
  });

  it("finds every occurrence in one line", () => {
    const hits = searchRows([["one two one", null]], "one");

    expect(hits).toEqual([
      { index: 0, side: "left", from: 0, to: 3 },
      { index: 0, side: "left", from: 8, to: 11 },
    ]);
  });

  it("ignores case, because nobody types the exact casing", () => {
    const hits = searchRows([["Alpha BETA", null]], "beta");

    expect(hits).toEqual([{ index: 0, side: "left", from: 6, to: 10 }]);
  });

  // DF-054: "İ" lowers to two code units, and offsets taken in the lowered text marked
  // every hit after it one character to the right.
  it("gives offsets in the line as written, whatever lowering it does", () => {
    expect(searchRows([["İstanbul foo", null]], "foo")).toEqual([{ index: 0, side: "left", from: 9, to: 12 }]);
    expect(searchRows([["İİ Foo", null]], "foo")).toEqual([{ index: 0, side: "left", from: 3, to: 6 }]);
  });

  it("finds brackets and dots as they are typed", () => {
    expect(searchRows([["a(b).c* [x]", null]], "(b).c*")).toEqual([{ index: 0, side: "left", from: 1, to: 7 }]);
  });

  it("reports which column the hit is in", () => {
    const hits = searchRows([["nothing", "needle"]], "needle");

    expect(hits).toEqual([{ index: 0, side: "right", from: 0, to: 6 }]);
  });

  it("returns hits in reading order, left column before right", () => {
    const hits = searchRows(
      [
        ["x", "x"],
        ["x", null],
      ],
      "x",
    );

    expect(hits.map((hit) => [hit.index, hit.side])).toEqual([
      [0, "left"],
      [0, "right"],
      [1, "left"],
    ]);
  });

  it("skips a padding cell that has no text", () => {
    expect(searchRows([[null, null]], "x")).toEqual([]);
  });

  it("finds nothing when the query is not there", () => {
    expect(searchRows([["alpha", "beta"]], "gamma")).toEqual([]);
  });

  it("does not spin on a query longer than the line", () => {
    expect(searchRows([["ab", null]], "abcdef")).toEqual([]);
  });

  it("treats overlapping candidates as successive, not nested", () => {
    const hits = searchRows([["aaaa", null]], "aa");

    expect(hits).toEqual([
      { index: 0, side: "left", from: 0, to: 2 },
      { index: 0, side: "left", from: 2, to: 4 },
    ]);
  });
});

describe("stepping through search hits", () => {
  const hits = [
    { index: 0, side: "left" as const, from: 0, to: 1 },
    { index: 4, side: "left" as const, from: 0, to: 1 },
    { index: 9, side: "left" as const, from: 0, to: 1 },
  ];

  it("wraps forward from the last hit to the first", () => {
    expect(stepHit(hits, 2, 1)).toBe(0);
  });

  it("wraps backward from the first hit to the last", () => {
    expect(stepHit(hits, 0, -1)).toBe(2);
  });

  it("moves one at a time in between", () => {
    expect(stepHit(hits, 0, 1)).toBe(1);
    expect(stepHit(hits, 2, -1)).toBe(1);
  });

  it("stays at nothing when there are no hits", () => {
    expect(stepHit([], 0, 1)).toBe(-1);
  });
});
