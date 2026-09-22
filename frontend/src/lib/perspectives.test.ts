import { describe, expect, it } from "vitest";
import {
  capFraction,
  DEFAULT_LAYOUT,
  DEFAULT_PERSPECTIVES,
  PANELS,
  isVisible,
  mergePerspectives,
  toggleHidden,
  type PanelId,
} from "./perspectives";

describe("DEFAULT_PERSPECTIVES", () => {
  it("has a Main perspective that shows everything", () => {
    expect(DEFAULT_PERSPECTIVES.main.hidden).toEqual([]);
  });

  it("has a Review perspective that clears the way for the diff", () => {
    expect(DEFAULT_PERSPECTIVES.review.hidden.length).toBeGreaterThan(0);
    expect(DEFAULT_PERSPECTIVES.review.hidden).not.toContain("diff");
  });

  it("never hides every panel", () => {
    for (const perspective of Object.values(DEFAULT_PERSPECTIVES)) {
      expect(perspective.hidden.length).toBeLessThan(PANELS.length);
    }
  });
});

describe("isVisible", () => {
  const shown = { fractions: DEFAULT_PERSPECTIVES.main.fractions, hidden: ["refs" as PanelId] };

  it("hides a panel the perspective hides", () => {
    expect(isVisible(shown, null, "refs")).toBe(false);
  });

  it("shows a panel the perspective keeps", () => {
    expect(isVisible(shown, null, "diff")).toBe(true);
  });

  it("shows only the maximized panel", () => {
    expect(isVisible(shown, "graph", "graph")).toBe(true);
    expect(isVisible(shown, "graph", "diff")).toBe(false);
  });

  it("shows a maximized panel even if the perspective hides it", () => {
    expect(isVisible(shown, "refs", "refs")).toBe(true);
  });
});

describe("toggleHidden", () => {
  it("hides a visible panel", () => {
    expect(toggleHidden([], "diff")).toEqual(["diff"]);
  });

  it("shows a hidden one again", () => {
    expect(toggleHidden(["diff"], "diff")).toEqual([]);
  });

  it("refuses to hide the last visible panel", () => {
    const allButOne = PANELS.filter((panel) => panel !== "diff");
    expect(toggleHidden(allButOne, "diff")).toEqual(allButOne);
  });
});

describe("mergePerspectives", () => {
  it("returns the defaults for nothing stored", () => {
    expect(mergePerspectives(null)).toEqual(DEFAULT_PERSPECTIVES);
  });

  it("keeps stored fractions", () => {
    const stored = { main: { fractions: { graph: 0.5 }, hidden: [] } };
    expect(mergePerspectives(stored as never).main.fractions.graph).toBe(0.5);
  });

  it("clamps a fraction that would collapse a pane", () => {
    const stored = { main: { fractions: { graph: 0 }, hidden: [] } };
    expect(mergePerspectives(stored as never).main.fractions.graph).toBeGreaterThan(0);
  });

  it("drops a panel name that is no longer a panel", () => {
    const stored = { main: { fractions: {}, hidden: ["ghost"] } };
    expect(mergePerspectives(stored as never).main.hidden).toEqual([]);
  });

  it("fills in a perspective the stored data does not have", () => {
    const stored = { main: { fractions: {}, hidden: [] } };
    expect(mergePerspectives(stored as never).review).toEqual(DEFAULT_PERSPECTIVES.review);
  });

  it("survives stored data that is not an object", () => {
    expect(mergePerspectives("nonsense" as never)).toEqual(DEFAULT_PERSPECTIVES);
  });

  it("accepts a v1 layout, which was a bare set of fractions", () => {
    const merged = mergePerspectives({ graph: 0.5 } as never);
    expect(merged.main.fractions.graph).toBe(0.5);
  });
});

describe("filesSplit", () => {
  it("is part of the saved layout, so the Files divider survives a restart", () => {
    expect(DEFAULT_LAYOUT.filesSplit).toBeGreaterThan(0);
  });

  it("comes back from a stored layout that predates it", () => {
    const upgraded = mergePerspectives({ leftColumn: 0.3 });
    expect(upgraded.main.fractions.filesSplit).toBe(DEFAULT_LAYOUT.filesSplit);
    expect(upgraded.main.fractions.leftColumn).toBe(0.3);
  });
});

describe("the commit message panel", () => {
  it("is one of the panels the View menu can toggle", () => {
    expect(PANELS).toContain("commit");
  });

  it("is shown by default in the Main perspective", () => {
    expect(DEFAULT_PERSPECTIVES.main.hidden).not.toContain("commit");
  });

  it("is out of the way while reviewing", () => {
    expect(DEFAULT_PERSPECTIVES.review.hidden).toContain("commit");
  });

  it("has its own share of the column height", () => {
    expect(DEFAULT_LAYOUT.commitBox).toBeGreaterThan(0);
  });
});

describe("capFraction", () => {
  // 400 px column, 120 px the commit panel cannot do without: Files stops at 70%.
  it("stops the upper panel where the lower one would drop below its minimum", () => {
    expect(capFraction(0.85, 400, 120)).toBeCloseTo(0.7);
  });

  it("leaves a fraction alone while both panels fit", () => {
    expect(capFraction(0.5, 400, 120)).toBe(0.5);
  });

  it("falls back to the ordinary clamp before the container has been measured", () => {
    expect(capFraction(0.95, 0, 120)).toBe(0.88);
  });

  it("never squeezes the upper panel below the smallest fraction either", () => {
    expect(capFraction(0.5, 100, 120)).toBe(0.12);
  });
});

describe("the Worktrees panel", () => {
  it("is a panel of its own, shown in Main and hidden in Review", () => {
    expect(PANELS).toContain("worktrees");
    expect(DEFAULT_PERSPECTIVES.main.hidden).not.toContain("worktrees");
    expect(DEFAULT_PERSPECTIVES.review.hidden).toContain("worktrees");
  });

  it("shows up in a layout saved before it existed, with its default height", () => {
    const stored = { main: { fractions: { graph: 0.5 }, hidden: ["diff"] } };
    const merged = mergePerspectives(stored as never).main;
    expect(merged.hidden).toEqual(["diff"]);
    expect(merged.fractions.worktrees).toBe(DEFAULT_LAYOUT.worktrees);
  });
});
