import { describe, expect, it } from "vitest";
import type { DiffRow, Hunk } from "$lib/ipc";
import { hunkSelection, lineKey, selectedRange, toggleLine } from "./selection";

function del(old: number, text: string): DiffRow {
  return { kind: "delete", old, text, inline: [] };
}
function ins(nw: number, text: string): DiffRow {
  return { kind: "insert", new: nw, text, inline: [] };
}
function ctx(old: number, nw: number, text: string): DiffRow {
  return { kind: "context", old, new: nw, text };
}
function hunk(rows: DiffRow[]): Hunk {
  return { oldStart: 1, oldLines: 1, newStart: 1, newLines: 1, header: "@@", rows };
}

describe("lineKey", () => {
  it("keeps the two sides apart", () => {
    expect(lineKey(del(3, "x"))).not.toBe(lineKey(ins(3, "x")));
  });

  it("is null for a context line, which cannot be staged on its own", () => {
    expect(lineKey(ctx(1, 1, "same"))).toBeNull();
  });
});

describe("toggleLine", () => {
  it("adds a line that was not selected", () => {
    expect(toggleLine(new Set(), "d:3").has("d:3")).toBe(true);
  });

  it("removes a line that was selected", () => {
    expect(toggleLine(new Set(["d:3"]), "d:3").has("d:3")).toBe(false);
  });

  it("does not mutate the set it was given", () => {
    const before = new Set(["d:3"]);
    toggleLine(before, "i:4");
    expect(before.size).toBe(1);
  });
});

describe("hunkSelection", () => {
  it("selects every changed line of a hunk and no context", () => {
    const keys = hunkSelection(hunk([ctx(1, 1, "keep"), del(2, "old"), ins(2, "new")]));

    expect(keys).toEqual(new Set(["d:2", "i:2"]));
  });

  it("is empty for a hunk of pure context", () => {
    expect(hunkSelection(hunk([ctx(1, 1, "keep")])).size).toBe(0);
  });

  it("splits the keys back into the two lists the backend expects", () => {
    const keys = hunkSelection(hunk([del(2, "old"), ins(5, "new"), ins(6, "more")]));
    const deletes = [...keys].filter((k) => k.startsWith("d:")).map((k) => Number(k.slice(2)));
    const inserts = [...keys].filter((k) => k.startsWith("i:")).map((k) => Number(k.slice(2)));

    expect(deletes).toEqual([2]);
    expect(inserts.sort()).toEqual([5, 6]);
  });
});

describe("selectedRange", () => {
  it("has no range when nothing is selected", () => {
    expect(selectedRange(new Set())).toBeNull();
  });

  it("spans the selected lines on the new side", () => {
    expect(selectedRange(new Set(["i:12", "i:9", "i:10"]))).toEqual({ from: 9, to: 12 });
  });

  it("prefers the new side, which is how the file is numbered now", () => {
    expect(selectedRange(new Set(["d:100", "i:4"]))).toEqual({ from: 4, to: 4 });
  });

  it("falls back to the old side when only deletions are selected", () => {
    expect(selectedRange(new Set(["d:7", "d:8"]))).toEqual({ from: 7, to: 8 });
  });

  it("collapses a single line into a range of one", () => {
    expect(selectedRange(new Set(["i:3"]))).toEqual({ from: 3, to: 3 });
  });

  it("spans the gap when the selection is not contiguous", () => {
    expect(selectedRange(new Set(["i:2", "i:40"]))).toEqual({ from: 2, to: 40 });
  });
});
