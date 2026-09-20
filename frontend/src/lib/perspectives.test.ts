import { describe, expect, it } from "vitest";
import {
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
