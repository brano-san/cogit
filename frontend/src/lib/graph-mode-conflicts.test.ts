import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS } from "./settings";
import { blockedModes, MODE_CONFLICTS, type ModeConflict } from "./graph-mode-conflicts";
import { MODE_CONFLICTS as GRAPH_CONFLICTS } from "./graph-modes";

const table: ModeConflict[] = [
  { mode: "graphCollapseMerged", by: "graphFirstParent", reason: "Nothing merged is listed." },
];

describe("blockedModes", () => {
  it("reads the graph's own table", () => {
    expect(MODE_CONFLICTS).toHaveLength(GRAPH_CONFLICTS.length);
    expect(blockedModes({ ...DEFAULT_SETTINGS, graphFirstParent: true })).toEqual([
      {
        mode: "graphCollapseMerged",
        reason: "First parents only already leaves every merged branch out.",
      },
    ]);
  });

  it("blocks nothing while the blocking side is off", () => {
    expect(blockedModes(DEFAULT_SETTINGS, table)).toEqual([]);
    expect(blockedModes({ ...DEFAULT_SETTINGS, graphCollapseMerged: true }, table)).toEqual([]);
  });

  it("greys out the mode that does nothing, with the reason", () => {
    expect(blockedModes({ ...DEFAULT_SETTINGS, graphFirstParent: true }, table)).toEqual([
      { mode: "graphCollapseMerged", reason: "Nothing merged is listed." },
    ]);
  });

  it("leaves a mode live when a file already has it on, so it can be undone", () => {
    const both = { ...DEFAULT_SETTINGS, graphFirstParent: true, graphCollapseMerged: true };
    expect(blockedModes(both, table)).toEqual([]);
  });

  it("names a blocked mode once, whichever rule blocked it first", () => {
    const two: ModeConflict[] = [
      ...table,
      { mode: "graphCollapseMerged", by: "graphAncestry", reason: "second" },
    ];
    const state = { ...DEFAULT_SETTINGS, graphFirstParent: true, graphAncestry: true };
    expect(blockedModes(state, two)).toEqual([
      { mode: "graphCollapseMerged", reason: "Nothing merged is listed." },
    ]);
  });
});

describe("blockedModes under a coloring", () => {
  it("greys out the ancestry under Mergeable Coloring, which dims already", () => {
    expect(blockedModes({ ...DEFAULT_SETTINGS, graphColoring: "mergeable" })).toEqual([
      { mode: "graphAncestry", reason: "Mergeable Coloring already dims all but what a merge would bring." },
    ]);
    expect(blockedModes({ ...DEFAULT_SETTINGS, graphColoring: "varying" })).toEqual([]);
  });
});
