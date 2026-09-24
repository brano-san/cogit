import { describe, expect, it } from "vitest";
import { GRAPH_MODE_DEFAULTS, checkedTips, focusLane, graphView, paintRequest } from "$lib/graph-modes";
import { branchSlot } from "$lib/graph-style";

const branches = [
  { name: "topic", kind: "local" as const, oid: "t" },
  { name: "main", kind: "local" as const, oid: "m" },
  { name: "origin/topic", kind: "remote" as const, oid: "r" },
  { name: "unticked", kind: "local" as const, oid: "u" },
];
const ticked = new Set(["HEAD", "local:topic", "local:main", "remote:origin/topic"]);

describe("checkedTips", () => {
  it("lists the ticked branches by name, each with the slot of its name", () => {
    expect(checkedTips(branches, ticked)).toEqual([
      { name: "main", oid: "m", slot: branchSlot("main") },
      { name: "origin/topic", oid: "r", slot: branchSlot("topic") },
      { name: "topic", oid: "t", slot: branchSlot("topic") },
    ]);
  });

  it("gives a remote branch the colour of the local one it mirrors", () => {
    const [, remote, local] = checkedTips(branches, ticked);
    expect(remote!.slot).toBe(local!.slot);
  });
});

describe("paintRequest", () => {
  const tips = checkedTips(branches, ticked);

  it("colours the ticked branches by default", () => {
    expect(GRAPH_MODE_DEFAULTS.highlightChecked).toBe(true);
    expect(paintRequest(GRAPH_MODE_DEFAULTS, tips)?.tips).toHaveLength(3);
  });

  it("asks for nothing when switched off or when nothing is ticked", () => {
    expect(paintRequest({ ...GRAPH_MODE_DEFAULTS, highlightChecked: false }, tips)).toBeNull();
    expect(paintRequest(GRAPH_MODE_DEFAULTS, [])).toBeNull();
  });
});

describe("graphView", () => {
  it("walks everything by default and first parents only when asked", () => {
    expect(graphView(GRAPH_MODE_DEFAULTS)).toEqual({ firstParent: false });
    expect(graphView({ ...GRAPH_MODE_DEFAULTS, firstParent: true })).toEqual({ firstParent: true });
  });
});

describe("focusLane", () => {
  const on = { ...GRAPH_MODE_DEFAULTS, branchOfCommit: true };

  it("is the selected commit's lane, or the line clicked in its row", () => {
    expect(focusLane(on, "a", 3, null)).toBe(3);
    expect(focusLane(on, "a", 3, { oid: "a", lane: 8 })).toBe(8);
    expect(focusLane(on, "b", 3, { oid: "a", lane: 8 })).toBe(3);
  });

  it("is nothing with the mode off or nothing selected", () => {
    expect(focusLane(GRAPH_MODE_DEFAULTS, "a", 3, null)).toBeNull();
    expect(focusLane(on, null, 3, null)).toBeNull();
  });

  it("asks for lanes even with no branch ticked", () => {
    expect(paintRequest(on, [])).toEqual({ tips: [] });
  });
});
