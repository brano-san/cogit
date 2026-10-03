import { describe, expect, it } from "vitest";
import { blockConnectors } from "./diff-band";
import { buildBlocks, type PaneLine } from "./diff-blocks";
import type { FoldEntry } from "./diff-fold";
import type { DiffRow } from "./ipc";

/**
 * The reference case of the diff spec, on a C++ file modelled on `algo_task.cpp`:
 * left 1064-1068 and right 1071-1079 are ONE changed block; each pane shows only its own
 * lines; the unchanged comment lines are not painted word by word; the appended
 * `std::back_inserter(heardAt),` is; the indentation only with the Whitespace option.
 */
const LEFT = [
  "        // walk the pending items",
  "        // in arrival order",
  "        std::transform(items.begin(), items.end(), std::back_inserter(result),",
  "            toResult);",
  "        stat.count += result.size();",
];
const RIGHT = [
  "        // walk the pending items",
  "        // in arrival order",
  "        // heardAt mirrors result, one entry per item",
  "        // (filled from the transport layer)",
  "    std::transform(items.begin(), items.end(), std::back_inserter(result), std::back_inserter(heardAt),",
  "        toResult);",
  "    stat.count += result.size();",
  "    stat.heard += heardAt.size();",
  "    flush();",
];

function entries(): FoldEntry[] {
  const rows: DiffRow[] = [];
  for (let i = 0; i < 4; i++) rows.push({ kind: "context", old: 1060 + i, new: 1067 + i, text: `    // context ${i}` });
  LEFT.forEach((text, i) => rows.push({ kind: "delete", old: 1064 + i, text, inline: [] }));
  RIGHT.forEach((text, i) => rows.push({ kind: "insert", new: 1071 + i, text, inline: [] }));
  for (let i = 0; i < 3; i++) rows.push({ kind: "context", old: 1069 + i, new: 1080 + i, text: `    // after ${i}` });
  return rows.map((row) => ({ kind: "row", row, block: 0 }));
}

const covered = (line: PaneLine) => line.inline.map(([a, b]) => line.text.slice(a, b));

describe("algo_task.cpp: 1064-1068 against 1071-1079", () => {
  const model = buildBlocks(entries());

  it("is exactly one changed block", () => {
    const changed = model.blocks.filter((b) => b.kind !== "equal");
    expect(changed).toHaveLength(1);
    expect(changed[0]).toMatchObject({ kind: "changed", leftStart: 1064, leftLen: 5, rightStart: 1071, rightLen: 9 });
  });

  it("gives each pane only its own lines, with no filler", () => {
    expect(model.left).toHaveLength(4 + 5 + 3);
    expect(model.right).toHaveLength(4 + 9 + 3);
    expect(model.left.every((r) => r.kind === "line")).toBe(true);
    expect(model.right.every((r) => r.kind === "line")).toBe(true);
  });

  it("draws one connector with one button pair, on the connector", () => {
    const view = { scrollTop: 0, viewport: 600 };
    const cs = blockConnectors(model, view, view, 18);
    expect(cs).toHaveLength(1);
    expect(cs[0]).toMatchObject({ left: [72, 162], right: [72, 234] });
    expect(cs[0]!.anchor.left).not.toBeNull();
    expect(cs[0]!.anchor.right).not.toBeNull();
    expect(cs[0]!.anchor.left!).toBeGreaterThanOrEqual(72);
    expect(cs[0]!.anchor.right!).toBeLessThanOrEqual(234);
  });

  it("highlights the appended call and not the comment lines", () => {
    const right = model.right.slice(4, 13) as PaneLine[];
    expect(right[0]!.inline).toEqual([]);
    expect(right[1]!.inline).toEqual([]);
    expect(right[2]!.inline).toEqual([]); // a wholly new line keeps only its background
    expect(right[3]!.inline).toEqual([]);
    expect(covered(right[4]!)).toEqual(["std::back_inserter(heardAt),"]);
    const left = model.left.slice(4, 9) as PaneLine[];
    expect(left.map(covered)).toEqual([[], [], [], [], []]);
  });

  it("marks the changed indentation only with the Whitespace option", () => {
    const on = buildBlocks(entries(), { indent: true });
    const line = on.right[4 + 4] as PaneLine;
    expect(covered(line)).toEqual(["    ", "std::back_inserter(heardAt),"]);
    expect(covered(on.right[4 + 5] as PaneLine)).toEqual(["        "]);
  });
});
