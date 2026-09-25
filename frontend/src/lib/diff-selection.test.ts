import { describe, expect, it } from "vitest";
import type { DiffRow, Hunk } from "$lib/ipc";
import { keepSelection } from "./diff-selection";

function context(old: number, nw: number, text: string): DiffRow {
  return { kind: "context", old, new: nw, text };
}
function del(old: number, text: string): DiffRow {
  return { kind: "delete", old, text, inline: [], moved: false };
}
function ins(nw: number, text: string): DiffRow {
  return { kind: "insert", new: nw, text, inline: [], moved: false };
}
function hunk(...rows: DiffRow[]): Hunk {
  return { oldStart: 0, oldLines: 0, newStart: 0, newLines: 0, header: "", rows };
}

// Working tree against the index: `a` replaced by `A` at the top, `x` removed further down.
const before = [hunk(del(1, "a"), ins(1, "A"), context(2, 2, "b")), hunk(context(5, 5, "e"), del(6, "x"))];

describe("keepSelection", () => {
  it("keeps every line of a diff that did not change", () => {
    const selected = new Set(["d:1", "i:1", "d:6"]);

    expect(keepSelection(selected, before, before)).toEqual(selected);
  });

  // Stage on the top block: the index now has `A`, so the old side shifted under nothing,
  // but a block that grew the index moves every deletion below it to another number.
  it("drops a deletion whose number now names another line", () => {
    const after = [hunk(context(5, 5, "e"), del(6, "y"), del(7, "x"))];

    expect(keepSelection(new Set(["d:6"]), before, after)).toEqual(new Set());
  });

  it("drops a line that is no longer part of any change", () => {
    const after = [hunk(context(5, 5, "e"), del(6, "x"))];

    expect(keepSelection(new Set(["d:1", "i:1", "d:6"]), before, after)).toEqual(new Set(["d:6"]));
  });

  it("drops a line that kept its number but moved against the other side", () => {
    const after = [hunk(del(1, "a"), ins(1, "A"), ins(2, "new"), context(2, 3, "b")), hunk(context(5, 6, "e"), del(6, "x"))];

    expect(keepSelection(new Set(["d:6"]), before, after)).toEqual(new Set());
  });

  it("keeps the selection when only more context came with the diff", () => {
    const wider = [
      hunk(del(1, "a"), ins(1, "A"), context(2, 2, "b"), context(3, 3, "c"), context(4, 4, "d"), context(5, 5, "e"), del(6, "x")),
    ];

    expect(keepSelection(new Set(["i:1", "d:6"]), before, wider)).toEqual(new Set(["i:1", "d:6"]));
  });

  it("starts empty from an empty selection", () => {
    expect(keepSelection(new Set(), before, [])).toEqual(new Set());
  });
});
