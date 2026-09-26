import { describe, expect, it } from "vitest";
import { GRAPH_COLORINGS, migratedColoring } from "./graph-coloring";

describe("graph colorings", () => {
  it("are SmartGit's four, in its menu order", () => {
    expect(GRAPH_COLORINGS).toEqual(["default", "branch", "mergeable", "varying"]);
  });

  it("take over from the two switches an older settings file has", () => {
    expect(migratedColoring({ coloredLanes: true })).toBe("varying");
    expect(migratedColoring({ graphBranchOfCommit: true })).toBe("branch");
    expect(migratedColoring({ coloredLanes: true, graphBranchOfCommit: true })).toBe("varying");
    expect(migratedColoring({ coloredLanes: false, graphBranchOfCommit: false })).toBeUndefined();
  });

  it("leave a coloring already chosen as it is", () => {
    expect(migratedColoring({ graphColoring: "mergeable", coloredLanes: true })).toBeUndefined();
  });
});
