import { describe, expect, it } from "vitest";
import { buildBlocks, changeRow, changes, hunkKeys } from "./diff-blocks";
import type { FoldEntry, Gap } from "./diff-fold";
import type { DiffRow } from "./ipc";

const ctx = (old: number, nw: number, text = `c${old}`): DiffRow => ({ kind: "context", old, new: nw, text });
const del = (old: number, text = `d${old}`, extra: Partial<Extract<DiffRow, { kind: "delete" }>> = {}): DiffRow => ({
  kind: "delete", old, text, inline: [], ...extra,
});
const ins = (nw: number, text = `i${nw}`, extra: Partial<Extract<DiffRow, { kind: "insert" }>> = {}): DiffRow => ({
  kind: "insert", new: nw, text, inline: [], ...extra,
});
const rows = (...r: DiffRow[]): FoldEntry[] => r.map((row) => ({ kind: "row", row, block: 0 }));
const gap = (oldFrom: number, oldTo: number, newFrom: number): FoldEntry => ({
  kind: "gap",
  gap: { oldFrom, oldTo, newFrom, hidden: oldTo - oldFrom + 1, loaded: true, up: true, down: true, context: null } as Gap,
});
const shape = (e: FoldEntry[]) =>
  buildBlocks(e).blocks.map((b) => [b.kind, b.leftStart, b.leftLen, b.rightStart, b.rightLen]);

describe("blocks", () => {
  it("merges adjacent deletes and inserts into one changed block", () => {
    expect(shape(rows(ctx(1, 1), del(2), del(3), ins(2), ins(3), ins(4), ctx(4, 5)))).toEqual([
      ["equal", 1, 1, 1, 1],
      ["changed", 2, 2, 2, 3],
      ["equal", 4, 1, 5, 1],
    ]);
  });

  it("keeps pure deletes and inserts apart and anchors the empty side", () => {
    expect(shape(rows(ctx(1, 1), del(2), ctx(3, 2), ins(3), ctx(4, 4)))).toEqual([
      ["equal", 1, 1, 1, 1],
      ["removed", 2, 1, 2, 0],
      ["equal", 3, 1, 2, 1],
      ["added", 4, 0, 3, 1],
      ["equal", 4, 1, 4, 1],
    ]);
  });

  it("handles an insert at the start, a delete at the end and an empty file", () => {
    expect(shape(rows(ins(1), ctx(1, 2)))[0]).toEqual(["added", 1, 0, 1, 1]);
    expect(shape(rows(ctx(1, 1), del(2)))[1]).toEqual(["removed", 2, 1, 2, 0]);
    expect(shape(rows(ins(1), ins(2)))).toEqual([["added", 1, 0, 1, 2]]);
    expect(shape([])).toEqual([]);
  });

  it("does not merge across a fold, and the fold is one row on both panes", () => {
    const m = buildBlocks([...rows(del(1), ins(1)), gap(2, 10, 2), ...rows(del(11), ins(11))]);
    expect(m.blocks.map((b) => b.kind)).toEqual(["changed", "equal", "changed"]);
    expect(m.blocks[1]).toMatchObject({ leftStart: 2, leftLen: 9, rightStart: 2, rightLen: 9, leftRows: 1, rightRows: 1 });
    expect(m.left.map((r) => r.kind)).toEqual(["line", "gap", "line"]);
    expect(m.blocks[2]).toMatchObject({ leftRow: 2, rightRow: 2 });
  });

  it("gives each pane only its own lines, with its own numbers and no filler", () => {
    const m = buildBlocks(rows(ctx(1, 1), del(2), del(3), ins(2), ins(3), ins(4), ctx(4, 5)));
    expect(m.left).toHaveLength(4);
    expect(m.right).toHaveLength(5);
    expect(m.left.map((r) => (r.kind === "line" ? r.line : 0))).toEqual([1, 2, 3, 4]);
    expect(m.right.map((r) => (r.kind === "line" ? r.line : 0))).toEqual([1, 2, 3, 4, 5]);
    expect(m.blocks[1]).toMatchObject({ leftRow: 1, leftRows: 2, rightRow: 1, rightRows: 3 });
    expect(m.left[1]).toMatchObject({ block: 1, blockKind: "changed", key: "d:2" });
    expect(m.right[3]).toMatchObject({ block: 1, blockKind: "changed", key: "i:4" });
    expect(m.blocks[1]!.keys).toEqual({ deletes: ["d:2", "d:3"], inserts: ["i:2", "i:3", "i:4"] });
  });

  it("lists a block's lines consecutively in Unified, deleted first, with no filler", () => {
    const m = buildBlocks(rows(ctx(1, 1), del(2), del(3), ins(2), ins(3), ins(4), ctx(4, 5)));
    expect(m.unified.map((r) => (r.kind === "line" ? r.type[0] : "g")).join("")).toBe("cddiiic");
    expect(m.blocks[1]).toMatchObject({ unifiedRow: 1, unifiedRows: 5 });
  });

  it("keeps moved blocks as their own kind and links the two ends", () => {
    const mv = { moved: true, moveId: 7 };
    const m = buildBlocks(rows(del(1, "a", mv), ctx(2, 1), ctx(3, 2), ins(3, "a", mv)));
    expect(m.blocks.map((b) => b.kind)).toEqual(["moved", "equal", "moved"]);
    expect(m.blocks[0]).toMatchObject({ leftStart: 1, leftLen: 1, rightStart: 1, rightLen: 0, moveId: 7 });
    expect(m.blocks[2]).toMatchObject({ leftLen: 0, rightStart: 3, rightLen: 1, moveId: 7 });
    expect(m.moves).toEqual([{ moveId: 7, left: 0, right: 2 }]);
  });

  it("splits a moved line from the plain change next to it", () => {
    const m = buildBlocks(rows(del(1, "m", { moved: true, moveId: 1 }), del(2), ins(2)));
    expect(m.blocks.map((b) => b.kind)).toEqual(["moved", "changed"]);
    expect(m.blocks[1]).toMatchObject({ leftStart: 2, leftLen: 1, rightStart: 2, rightLen: 1 });
  });

  it("fills word ranges of a changed block and honors the indent option", () => {
    const e = rows(del(1, "  foo(x);"), ins(1, "    foo(x);"));
    expect(buildBlocks(e).right[0]).toMatchObject({ inline: [] });
    expect(buildBlocks(e, { indent: true }).right[0]).toMatchObject({ inline: [[0, 4]] });
  });

  it("is linear on a large file", () => {
    const list: DiffRow[] = [];
    for (let i = 1; i <= 200_000; i++) list.push(i % 50 === 0 ? del(i) : ctx(i, i));
    const t0 = performance.now();
    const m = buildBlocks(list.map((row): FoldEntry => ({ kind: "row", row, block: 0 })));
    expect(performance.now() - t0).toBeLessThan(300);
    expect(m.blocks.length).toBeGreaterThan(7000);
  });
});

describe("hunks and changes", () => {
  const entries: FoldEntry[] = [
    { kind: "row", row: ctx(1, 1), block: 0 },
    { kind: "row", row: del(2), block: 0 },
    { kind: "row", row: ins(2), block: 0 },
    { kind: "row", row: ctx(3, 3), block: 0 },
    gap(4, 9, 4),
    { kind: "row", row: del(10), block: 1 },
    { kind: "row", row: ctx(11, 10), block: 1 },
  ];

  it("collects the keys of one fold-block for its hunk actions", () => {
    const model = buildBlocks(entries);
    expect(hunkKeys(model, 0)).toEqual(new Set(["d:2", "i:2"]));
    expect(hunkKeys(model, 1)).toEqual(new Set(["d:10"]));
  });

  it("lists each run of non-equal blocks once, with the row it starts at", () => {
    const model = buildBlocks(entries);
    const found = changes(model);
    expect(found).toHaveLength(2);
    expect(changeRow(model, found[0]!, false)).toBe(model.blocks[found[0]!.from]!.leftRow);
    expect(changeRow(model, found[1]!, true)).toBe(model.blocks[found[1]!.from]!.unifiedRow);
  });

  it("merges a move that touches a plain change into one stop", () => {
    const model = buildBlocks(
      rows(ctx(1, 1), del(2), del(3, "m", { moved: true, moveId: 1 }), ctx(4, 2)),
    );
    expect(changes(model)).toHaveLength(1);
  });
});
