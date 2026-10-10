import { describe, expect, it } from "vitest";
import type { PaneRow, UnifiedRow } from "./diff-blocks";
import { splitRows, unifiedRows, type RowLook } from "./diff-edit-rows";
import type { Gap } from "./diff-fold";

const GAP: Gap = { oldFrom: 1, oldTo: 4, newFrom: 1, hidden: 4, loaded: true, up: true, down: false, context: null };

function look(over: Partial<RowLook> = {}): RowLook {
  return {
    selected: new Set(),
    stageable: true,
    flash: null,
    hits: () => [],
    current: () => false,
    fold: (_gap, index) => ({ id: `fold ${index}`, render: () => () => {} }),
    ...over,
  };
}

const line = (n: number, extra: Partial<Extract<PaneRow, { kind: "line" }>> = {}): PaneRow => ({
  kind: "line",
  line: n,
  text: "text",
  block: 1,
  blockKind: "changed",
  moveId: null,
  noNewline: false,
  key: `i:${n}`,
  inline: [],
  ...extra,
});

describe("splitRows", () => {
  it("draws the right pane's lines, folds and filler as the file's rows", () => {
    const rows: PaneRow[] = [{ kind: "gap", gap: GAP, block: 0 }, line(5), { kind: "filler", block: 1, blockKind: "changed" }];

    const out = splitRows(rows, look());

    expect(out[0]).toMatchObject({ kind: "fold", from: 1, hidden: 4, id: "fold 0" });
    expect(out[1]).toMatchObject({ kind: "code", line: 5, tone: "changed", key: "i:5", pick: null, flash: false });
    expect(out[2]).toEqual({ kind: "filler" });
  });

  it("marks the selected lines, the flashed block, changed words and search hits", () => {
    const rows = [line(5, { inline: [[0, 2]] })];
    const view = look({
      selected: new Set(["i:5"]),
      flash: { from: 1, to: 1 },
      hits: () => [[1, 3]],
      current: (_index, start) => start === 1,
    });

    expect(splitRows(rows, view)[0]).toMatchObject({
      pick: "staging",
      flash: true,
      marks: [
        { from: 0, to: 2, cls: "word" },
        { from: 1, to: 3, cls: "hit current" },
      ],
    });
    expect(splitRows(rows, look({ selected: new Set(["i:5"]), stageable: false }))[0]).toMatchObject({ pick: "marked" });
  });
});

describe("unifiedRows", () => {
  const unified = (type: "context" | "delete" | "insert", old: number | null, next: number | null): UnifiedRow => ({
    kind: "line",
    type,
    old,
    new: next,
    text: "gone",
    block: 1,
    blockKind: type === "context" ? "equal" : "changed",
    moveId: null,
    noNewline: false,
    key: type === "delete" ? `d:${old}` : type === "insert" ? `i:${next}` : null,
    inline: type === "delete" ? [[0, 2]] : [],
  });

  it("keeps a deleted line out of the file, with its old number and its own colouring", () => {
    const out = unifiedRows([unified("context", 1, 1), unified("delete", 2, null), unified("insert", null, 2)], look(), () => [
      { start: 2, end: 4, cls: "tok-keyword" },
    ]);

    expect(out[0]).toMatchObject({ kind: "code", line: 1, old: 1, tone: "" });
    expect(out[1]).toMatchObject({
      kind: "deleted",
      old: 2,
      tone: "del",
      key: "d:2",
      pieces: [
        { text: "go", cls: "word" },
        { text: "ne", cls: "tok-keyword" },
      ],
    });
    expect(out[2]).toMatchObject({ kind: "code", line: 2, old: null, tone: "add", key: "i:2" });
  });
});
