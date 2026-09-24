import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS } from "./settings";
import { blockedModes, MODE_CONFLICTS, type ModeConflict } from "./graph-mode-conflicts";

const table: ModeConflict[] = [
  { a: "graphFirstParent", b: "graphCollapseMerged", reason: "Nothing merged is listed." },
];

describe("blockedModes", () => {
  it("blocks nothing with the placeholder table", () => {
    expect(MODE_CONFLICTS).toEqual([]);
    expect(blockedModes({ ...DEFAULT_SETTINGS, graphFirstParent: true })).toEqual([]);
  });

  it("blocks nothing while neither side is on", () => {
    expect(blockedModes(DEFAULT_SETTINGS, table)).toEqual([]);
  });

  it("greys out the other side of a conflict, with the reason", () => {
    expect(blockedModes({ ...DEFAULT_SETTINGS, graphFirstParent: true }, table)).toEqual([
      { mode: "graphCollapseMerged", reason: "Nothing merged is listed." },
    ]);
    expect(blockedModes({ ...DEFAULT_SETTINGS, graphCollapseMerged: true }, table)).toEqual([
      { mode: "graphFirstParent", reason: "Nothing merged is listed." },
    ]);
  });

  it("leaves both live when a file already has both on, so either can be undone", () => {
    const both = { ...DEFAULT_SETTINGS, graphFirstParent: true, graphCollapseMerged: true };
    expect(blockedModes(both, table)).toEqual([]);
  });

  it("names a blocked mode once, whichever rule blocked it first", () => {
    const two: ModeConflict[] = [
      ...table,
      { a: "graphAncestry", b: "graphCollapseMerged", reason: "second" },
    ];
    const state = { ...DEFAULT_SETTINGS, graphFirstParent: true, graphAncestry: true };
    expect(blockedModes(state, two)).toEqual([
      { mode: "graphCollapseMerged", reason: "Nothing merged is listed." },
    ]);
  });
});
