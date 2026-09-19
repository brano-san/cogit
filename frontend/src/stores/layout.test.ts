import { describe, expect, it } from "vitest";
import { clampFraction, DEFAULT_LAYOUT } from "./layout.svelte";

describe("clampFraction", () => {
  it("leaves sensible values untouched", () => {
    expect(clampFraction(0.5)).toBe(0.5);
    expect(clampFraction(0.25)).toBe(0.25);
  });

  it("stops a pane from collapsing to nothing", () => {
    // Dragging a splitter to the edge must not make a panel unrecoverable.
    expect(clampFraction(0)).toBeGreaterThan(0);
    expect(clampFraction(-3)).toBeGreaterThan(0);
  });

  it("stops a pane from swallowing the whole container", () => {
    expect(clampFraction(1)).toBeLessThan(1);
    expect(clampFraction(50)).toBeLessThan(1);
  });

  it("survives NaN from a corrupt stored layout", () => {
    expect(Number.isFinite(clampFraction(Number.NaN))).toBe(true);
    expect(Number.isFinite(clampFraction(Number.POSITIVE_INFINITY))).toBe(true);
  });

  it("keeps every default within the allowed range", () => {
    for (const value of Object.values(DEFAULT_LAYOUT)) {
      expect(clampFraction(value)).toBe(value);
    }
  });
});
