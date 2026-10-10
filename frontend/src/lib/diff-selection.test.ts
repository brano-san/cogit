import { describe, expect, it } from "vitest";
import type { DiffRow, Hunk } from "$lib/ipc";
import { discardsWholeNewFile } from "./diff-selection";

function context(old: number, nw: number, text: string): DiffRow {
  return { kind: "context", old, new: nw, text };
}
function ins(nw: number, text: string): DiffRow {
  return { kind: "insert", new: nw, text, inline: [], moved: false };
}
function hunk(...rows: DiffRow[]): Hunk {
  return { oldStart: 0, oldLines: 0, newStart: 0, newLines: 0, header: "", rows };
}

describe("discardsWholeNewFile (item 1)", () => {
  const all = [hunk(ins(1, "a"), ins(2, "b"))];
  const created = { oldTotal: 0, newTotal: 2, hunks: all };

  it("is true when every line of a file with no old side is selected", () => {
    expect(discardsWholeNewFile(created, new Set(["i:1", "i:2"]))).toBe(true);
  });

  it("is false for part of the lines", () => {
    expect(discardsWholeNewFile(created, new Set(["i:1"]))).toBe(false);
  });

  it("is false for a file that existed before, whatever is selected", () => {
    const old = { oldTotal: 2, newTotal: 3, hunks: [hunk(context(1, 1, "x"), ins(2, "a"))] };
    expect(discardsWholeNewFile(old, new Set(["i:2"]))).toBe(false);
  });
});
