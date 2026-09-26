import { describe, expect, it } from "vitest";
import {
  GRAPH_MODE_DEFAULTS,
  MODE_CONFLICTS,
  checkedTips,
  conflictingModes,
  effectiveModes,
  focusLane,
  graphView,
  paintRequest,
  viewReloads,
  walkedView,
} from "$lib/graph-modes";
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
    expect(graphView(GRAPH_MODE_DEFAULTS)).toEqual({ firstParent: false, collapseMerged: false, expanded: [] });
    expect(graphView({ ...GRAPH_MODE_DEFAULTS, firstParent: true })).toMatchObject({ firstParent: true });
  });

  it("sends the opened merges only while merged branches fold, in one order", () => {
    const opened = new Set(["b", "a"]);
    const folding = { ...GRAPH_MODE_DEFAULTS, collapseMerged: true };
    expect(graphView(folding, opened)).toEqual({ firstParent: false, collapseMerged: true, expanded: ["a", "b"] });
    expect(graphView(GRAPH_MODE_DEFAULTS, opened).expanded).toEqual([]);
  });
});

describe("viewReloads", () => {
  const plain = graphView(GRAPH_MODE_DEFAULTS);
  const lines = graphView({ ...GRAPH_MODE_DEFAULTS, filteredGraph: true });
  const folding = graphView({ ...GRAPH_MODE_DEFAULTS, collapseMerged: true });

  it("sends the lines only when they are on, so the whole history's walk is the same", () => {
    expect(plain).not.toHaveProperty("filteredGraph");
    expect(lines).toMatchObject({ filteredGraph: true });
  });

  it("leaves the lines out of a load without a filter", () => {
    expect(walkedView(lines, { path: null })).toEqual(plain);
    expect(walkedView(lines, { path: "src" })).toEqual(lines);
  });

  it("walks a filtered list again only for its lines", () => {
    expect(viewReloads(plain, lines, true)).toBe(true);
    expect(viewReloads(plain, folding, true)).toBe(false);
  });

  it("walks the whole history again for anything but the lines", () => {
    expect(viewReloads(plain, lines, false)).toBe(false);
    expect(viewReloads(plain, folding, false)).toBe(true);
  });
});

describe("focusLane", () => {
  const on = { ...GRAPH_MODE_DEFAULTS, coloring: "branch" as const };

  it("is the selected commit's lane, or the line clicked in its row", () => {
    expect(focusLane(on, "a", 3, null, "1:1")).toBe(3);
    expect(focusLane(on, "a", 3, { oid: "a", lane: 8, walk: "1:1" }, "1:1")).toBe(8);
    expect(focusLane(on, "b", 3, { oid: "a", lane: 8, walk: "1:1" }, "1:1")).toBe(3);
  });

  // Every walk numbers its lanes afresh: lane 8 of the last one is another branch now.
  it("forgets the line clicked once another walk replaced the one it was clicked in", () => {
    expect(focusLane(on, "a", 3, { oid: "a", lane: 8, walk: "1:1" }, "1:2")).toBe(3);
  });

  it("is nothing with the mode off or nothing selected", () => {
    expect(focusLane(GRAPH_MODE_DEFAULTS, "a", 3, null, "1:1")).toBeNull();
    expect(focusLane(on, null, 3, null, "1:1")).toBeNull();
  });

  it("asks for lanes even with no branch ticked", () => {
    expect(paintRequest(on, [])).toEqual({ tips: [] });
  });
});

describe("ancestry in the paint request", () => {
  const on = { ...GRAPH_MODE_DEFAULTS, ancestry: true };

  it("names the selected commit, and asks for nothing while none is selected", () => {
    expect(paintRequest(on, [], "abc")).toEqual({ tips: [], ancestryOf: "abc" });
    expect(paintRequest(on, [], null)).toBeNull();
    expect(paintRequest(GRAPH_MODE_DEFAULTS, [], "abc")).toBeNull();
  });
});

describe("conflicting modes", () => {
  const every = {
    highlightChecked: true,
    firstParent: true,
    coloring: "branch" as const,
    ancestry: true,
    collapseMerged: true,
    filteredGraph: true,
  };

  it("leave only folding merged branches out, while first parents hide them anyway", () => {
    expect(conflictingModes(every)).toEqual([
      { mode: "collapseMerged", reason: "First parents only already leaves every merged branch out." },
    ]);
    expect(effectiveModes(every)).toEqual({ ...every, collapseMerged: false });
    expect(graphView(every)).toEqual({ firstParent: true, collapseMerged: false, expanded: [], filteredGraph: true });
  });

  it("are none while the blocking mode is off", () => {
    expect(conflictingModes({ ...every, firstParent: false })).toEqual([]);
    expect(conflictingModes(GRAPH_MODE_DEFAULTS)).toEqual([]);
  });

  it("leave the ancestry out under Mergeable Coloring, which dims already", () => {
    const mergeable = { ...GRAPH_MODE_DEFAULTS, ancestry: true, coloring: "mergeable" as const };
    expect(conflictingModes(mergeable)).toEqual([
      { mode: "ancestry", reason: "Mergeable Coloring already dims all but what a merge would bring." },
    ]);
    expect(paintRequest(mergeable, [], "abc")).toEqual({ tips: [], mergeableOf: "abc" });
  });

  it("name real modes and never a mode against itself", () => {
    for (const { mode, by } of MODE_CONFLICTS) {
      expect(Object.keys(GRAPH_MODE_DEFAULTS)).toContain(mode);
      expect(Object.keys(GRAPH_MODE_DEFAULTS)).toContain(by);
      expect(mode).not.toBe(by);
    }
  });

  it("ask for folds even with nothing to colour", () => {
    expect(paintRequest({ ...GRAPH_MODE_DEFAULTS, highlightChecked: false, collapseMerged: true }, [])).toEqual({
      tips: [],
    });
  });
});
