import { describe, expect, it } from "vitest";
import type { DiffRow, Hunk } from "$lib/ipc";
import { flatten, pairRows } from "./diff-rows";

function context(old: number, nw: number, text: string): DiffRow {
  return { kind: "context", old, new: nw, text };
}
function del(old: number, text: string): DiffRow {
  return { kind: "delete", old, text, inline: [] };
}
function ins(nw: number, text: string): DiffRow {
  return { kind: "insert", new: nw, text, inline: [] };
}
function hunk(rows: DiffRow[], header = "@@ -1,1 +1,1 @@"): Hunk {
  return { oldStart: 1, oldLines: 1, newStart: 1, newLines: 1, header, rows };
}

describe("pairRows", () => {
  it("puts a context line on both sides", () => {
    const pairs = pairRows([context(1, 1, "same")]);

    expect(pairs).toHaveLength(1);
    expect(pairs[0]!.left).toEqual({ kind: "context", line: 1, text: "same", inline: [] });
    expect(pairs[0]!.right).toEqual({ kind: "context", line: 1, text: "same", inline: [] });
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
