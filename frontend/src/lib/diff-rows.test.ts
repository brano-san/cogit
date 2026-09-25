import { describe, expect, it } from "vitest";
import type { DiffRow, Hunk } from "$lib/ipc";
import {
  expandedContext,
  lacksFinalNewline,
  pairRows,
  connectors,
  searchRows,
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

describe("lacksFinalNewline", () => {
  function hunkOf(rows: DiffRow[]): Hunk {
    return { oldStart: 1, oldLines: 1, newStart: 1, newLines: 1, header: "@@", rows };
  }

  it("says no for a file that ends in a newline", () => {
    const rows: DiffRow[] = [{ kind: "insert", new: 1, text: "a", inline: [] }];
    expect(lacksFinalNewline([hunkOf(rows)])).toBe(false);
  });

  it("says yes when the last row is flagged", () => {
    const rows: DiffRow[] = [{ kind: "insert", new: 1, text: "a", inline: [], noNewline: true }];
    expect(lacksFinalNewline([hunkOf(rows)])).toBe(true);
  });

  it("looks only at the very last row", () => {
    const rows: DiffRow[] = [
      { kind: "insert", new: 1, text: "a", inline: [], noNewline: true },
      { kind: "insert", new: 2, text: "b", inline: [] },
    ];
    expect(lacksFinalNewline([hunkOf(rows)])).toBe(false);
  });

  it("ignores a context row at the end", () => {
    const rows: DiffRow[] = [{ kind: "context", old: 1, new: 1, text: "a" }];
    expect(lacksFinalNewline([hunkOf(rows)])).toBe(false);
  });

  it("says no when there are no hunks at all", () => {
    expect(lacksFinalNewline([])).toBe(false);
  });
});

describe("connectors between the two columns", () => {
  function cell(kind: "delete" | "insert" | "context", moveId: number | null = null) {
    return { kind, line: 1, text: "x", inline: [], moved: moveId !== null, moveId, noNewline: false };
  }
  const ctx = () => ({ left: cell("context"), right: cell("context") });
  const header = () => null;

  it("draws nothing when nothing changed", () => {
    expect(connectors([ctx(), ctx()])).toEqual([]);
  });

  it("ties one block of changes to the rows facing it", () => {
    const rows = [ctx(), { left: cell("delete"), right: cell("insert") }, ctx()];

    expect(connectors(rows)).toEqual([
      { fromTop: 1, fromBottom: 1, toTop: 1, toBottom: 1, moved: false },
    ]);
  });

  it("treats consecutive changed rows as one connector", () => {
    const rows = [
      ctx(),
      { left: cell("delete"), right: cell("insert") },
      { left: cell("delete"), right: null },
      { left: cell("delete"), right: null },
      ctx(),
    ];

    expect(connectors(rows)).toEqual([
      { fromTop: 1, fromBottom: 3, toTop: 1, toBottom: 3, moved: false },
    ]);
  });

  it("separates blocks that context rows keep apart", () => {
    const rows = [
      { left: cell("delete"), right: cell("insert") },
      ctx(),
      { left: cell("delete"), right: cell("insert") },
    ];

    expect(connectors(rows)).toHaveLength(2);
  });

  it("joins the two ends of a move however far apart they are", () => {
    const rows = [
      { left: cell("delete", 7), right: null },
      { left: cell("delete", 7), right: null },
      ctx(),
      ctx(),
      { left: null, right: cell("insert", 7) },
      { left: null, right: cell("insert", 7) },
    ];

    expect(connectors(rows)).toEqual([
      { fromTop: 0, fromBottom: 1, toTop: 4, toBottom: 5, moved: true },
    ]);
  });

  it("gives every move its own connector", () => {
    const rows = [
      { left: cell("delete", 1), right: null },
      { left: cell("delete", 2), right: null },
      ctx(),
      { left: null, right: cell("insert", 2) },
      { left: null, right: cell("insert", 1) },
    ];

    const drawn = connectors(rows);

    expect(drawn.filter((c) => c.moved)).toHaveLength(2);
  });

  it("does not draw a move whose other end is out of the diff", () => {
    const rows = [{ left: cell("delete", 3), right: null }, ctx()];

    expect(connectors(rows)).toEqual([
      { fromTop: 0, fromBottom: 0, toTop: 0, toBottom: 0, moved: false },
    ]);
  });

  it("leaves a row a move already claimed out of the plain connectors", () => {
    const rows = [
      { left: cell("delete", 5), right: null },
      ctx(),
      { left: null, right: cell("insert", 5) },
    ];

    const drawn = connectors(rows);

    expect(drawn).toHaveLength(1);
    expect(drawn[0]?.moved).toBe(true);
  });

  it("skips header rows without letting them join two blocks", () => {
    const rows = [
      { left: cell("delete"), right: cell("insert") },
      header(),
      { left: cell("delete"), right: cell("insert") },
    ];

    expect(connectors(rows)).toHaveLength(2);
  });
});

// The viewer never read `noNewline`: a change of nothing but the final newline showed as
// `− b` / `+ b`, the same text twice with nothing to tell them apart.
describe("pairRows and the final newline", () => {
  it("carries which cell ends its file without a newline", () => {
    const pairs = pairRows([
      { kind: "delete", old: 2, text: "b", inline: [], moved: false, noNewline: true },
      ins(2, "b"),
    ]);

    expect(pairs[0]?.left?.noNewline).toBe(true);
    expect(pairs[0]?.right?.noNewline).toBe(false);
  });

  it("carries it on a context line both sides end on", () => {
    const pairs = pairRows([{ kind: "context", old: 3, new: 3, text: "c", noNewline: true }]);

    expect(pairs[0]?.left?.noNewline).toBe(true);
    expect(pairs[0]?.right?.noNewline).toBe(true);
  });
});
