import { describe, expect, it } from "vitest";
import type { DiffRow, Hunk } from "$lib/ipc";
import {
  expandedContext,
  flatten,
  gapBetween,
  pairRows,
  searchRows,
  segments,
  stepHit,
} from "./diff-rows";

function context(old: number, nw: number, text: string): DiffRow {
  return { kind: "context", old, new: nw, text };
}
function del(old: number, text: string): DiffRow {
  return { kind: "delete", old, text, inline: [], moved: false };
}
function ins(nw: number, text: string): DiffRow {
  return { kind: "insert", new: nw, text, inline: [], moved: false };
}
function hunk(rows: DiffRow[], header = "@@ -1,1 +1,1 @@"): Hunk {
  return { oldStart: 1, oldLines: 1, newStart: 1, newLines: 1, header, rows };
}

describe("pairRows", () => {
  it("puts a context line on both sides", () => {
    const pairs = pairRows([context(1, 1, "same")]);

    expect(pairs).toHaveLength(1);
    expect(pairs[0]!.left).toMatchObject({ kind: "context", line: 1, text: "same" });
    expect(pairs[0]!.right).toMatchObject({ kind: "context", line: 1, text: "same" });
  });

  it("pairs a replaced line with its replacement", () => {
    const pairs = pairRows([del(2, "before"), ins(2, "after")]);

    expect(pairs).toHaveLength(1);
    expect(pairs[0]!.left?.kind).toBe("delete");
    expect(pairs[0]!.left?.text).toBe("before");
    expect(pairs[0]!.right?.kind).toBe("insert");
    expect(pairs[0]!.right?.text).toBe("after");
  });

  it("leaves the right side empty for a pure deletion", () => {
    const pairs = pairRows([del(5, "gone")]);

    expect(pairs[0]!.left?.text).toBe("gone");
    expect(pairs[0]!.right).toBeNull();
  });

  it("leaves the left side empty for a pure insertion", () => {
    const pairs = pairRows([ins(5, "new")]);

    expect(pairs[0]!.left).toBeNull();
    expect(pairs[0]!.right?.text).toBe("new");
  });

  it("pads the shorter side when a block grows", () => {
    const pairs = pairRows([del(1, "a"), ins(1, "a1"), ins(2, "a2"), ins(3, "a3")]);

    expect(pairs).toHaveLength(3);
    expect(pairs.map((p) => p.left?.text ?? null)).toEqual(["a", null, null]);
    expect(pairs.map((p) => p.right?.text ?? null)).toEqual(["a1", "a2", "a3"]);
  });

  it("pads the shorter side when a block shrinks", () => {
    const pairs = pairRows([del(1, "a"), del(2, "b"), del(3, "c"), ins(1, "abc")]);

    expect(pairs).toHaveLength(3);
    expect(pairs.map((p) => p.right?.text ?? null)).toEqual(["abc", null, null]);
  });

  it("keeps separate change blocks separate", () => {
    const pairs = pairRows([
      del(1, "a"),
      ins(1, "A"),
      context(2, 2, "keep"),
      del(3, "b"),
      ins(3, "B"),
    ]);

    expect(pairs).toHaveLength(3);
    expect(pairs[1]!.left?.kind).toBe("context");
  });

  it("does not let an insertion after context absorb the earlier deletion", () => {
    const pairs = pairRows([del(1, "a"), context(2, 1, "keep"), ins(2, "z")]);

    expect(pairs).toHaveLength(3);
    expect(pairs[0]!.right).toBeNull();
    expect(pairs[2]!.left).toBeNull();
  });

  it("handles an empty row list", () => {
    expect(pairRows([])).toEqual([]);
  });
});

describe("flatten", () => {
  it("emits a header entry before each hunk", () => {
    const rows = flatten([hunk([context(1, 1, "a")], "@@ -1 +1 @@"), hunk([context(9, 9, "b")])]);

    expect(rows[0]).toMatchObject({ kind: "header", text: "@@ -1 +1 @@" });
    expect(rows[1]).toMatchObject({ kind: "row" });
    expect(rows[2]).toMatchObject({ kind: "header" });
  });

  it("numbers hunks so navigation can jump between them", () => {
    const rows = flatten([hunk([context(1, 1, "a")]), hunk([context(9, 9, "b")])]);
    const headers = rows.filter((r) => r.kind === "header");

    expect(headers.map((h) => h.hunk)).toEqual([0, 1]);
  });

  it("gives every entry the index of the hunk it belongs to", () => {
    const rows = flatten([hunk([context(1, 1, "a"), del(2, "b")]), hunk([ins(9, "c")])]);

    expect(rows.map((r) => r.hunk)).toEqual([0, 0, 0, 1, 1]);
  });

  it("produces nothing for no hunks", () => {
    expect(flatten([])).toEqual([]);
  });
});

describe("segments", () => {
  it("returns the whole line unchanged when there are no spans", () => {
    expect(segments("hello", [])).toEqual([{ text: "hello", changed: false }]);
  });

  it("splits a line around one changed word", () => {
    expect(segments("the quick fox", [[4, 9]])).toEqual([
      { text: "the ", changed: false },
      { text: "quick", changed: true },
      { text: " fox", changed: false },
    ]);
  });

  it("handles a span at the very start", () => {
    expect(segments("abc", [[0, 1]])).toEqual([
      { text: "a", changed: true },
      { text: "bc", changed: false },
    ]);
  });

  it("handles a span running to the end", () => {
    expect(segments("abc", [[1, 3]])).toEqual([
      { text: "a", changed: false },
      { text: "bc", changed: true },
    ]);
  });

  it("keeps several spans in order", () => {
    expect(segments("a b c", [[0, 1], [4, 5]]).map((s) => s.changed)).toEqual([
      true,
      false,
      true,
    ]);
  });

  it("slices surrogate pairs by UTF-16 offset, as the backend counts them", () => {
    expect(segments("привет 🙂 мир", [[10, 13]])).toEqual([
      { text: "привет 🙂 ", changed: false },
      { text: "мир", changed: true },
    ]);
  });

  it("clamps a span that runs past the end rather than producing undefined", () => {
    expect(segments("ab", [[1, 99]])).toEqual([
      { text: "a", changed: false },
      { text: "b", changed: true },
    ]);
  });
});

describe("gapBetween", () => {
  const hunk = (oldStart: number, oldLines: number): Hunk =>
    ({ oldStart, oldLines, newStart: oldStart, newLines: oldLines, header: "", rows: [] }) as Hunk;

  it("counts the lines between two hunks", () => {
    expect(gapBetween(hunk(1, 5), hunk(30, 5))).toBe(24);
  });

  it("is zero for hunks that touch", () => {
    expect(gapBetween(hunk(1, 5), hunk(6, 5))).toBe(0);
  });

  it("is zero rather than negative for overlapping hunks", () => {
    expect(gapBetween(hunk(1, 20), hunk(5, 5))).toBe(0);
  });

  it("counts the lines above the first hunk", () => {
    expect(gapBetween(null, hunk(10, 3))).toBe(9);
  });

  it("is zero when the first hunk starts at the top", () => {
    expect(gapBetween(null, hunk(1, 3))).toBe(0);
  });
});

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
